//! The gene ledger: what the screen has measured about every gene, and the
//! deployment genome the batch rule — under the operator's pins — decides
//! from it.
//!
//! ⚠ Names in this module doc are code spans, not intra-doc links, and have to
//! stay that way: `ai::advanced` is a private module, so `cargo doc` documents
//! neither it nor this module, and a link from a `//!` doc inside it cannot
//! resolve — not as a bare name, not as `self::`. Items *within* the module,
//! whose docs hang off `AdvancedAi`, link normally.
//!
//! ⭐ ONE SCREEN. Operator directive 2026-08-22: every row here comes from the
//! same shape — six majors on 74x46 continents with nine city-states, Online
//! speed to its own 250-turn clock, all six victory lanes, every seat carrying
//! its own drawn genome against the best-genome baseline. There is no second
//! regime to reconcile, and `tools/genes.py` refuses a source played at
//! any other profile. The columns carried over from the pre-2026-08-22 Pangaea
//! screens are marked `legacy` in `docs/gene_ledger.json`: history retained
//! as evidence beside the batch rule's answer.
//!
//! ★★★★ THE DEFAULT FOLLOWS THE BATCH RULE (operator, 2026-08-25). Every
//! screenable gene's default is decided by its three batch columns in
//! `GENE_HEURISTIC_RANKING.md` — *Last Batch*, *Prior Batch*, *Third Batch*,
//! the ± wins per 10,000 total seats each fixed reporting batch read for the
//! gene, newest first — through `batch_rule` below, the operator's words as a
//! function, and by nothing else:
//!
//! 1. three batches all below −10 → the gene is REMOVED from the gene pool;
//! 2. two or three batches negative → off;
//! 3. three batches all positive → on;
//! 4. three batches, exactly two positive, and their mean > 7 → on;
//! 5. one or two batches, exactly one positive, and their mean > 7 → on;
//! 6. two batches, both positive → on (all of its batches are positive);
//! 7. otherwise off — a gene no batch has priced is off, and a reading of
//!    exactly zero is neither positive nor negative.
//!
//! `tools/genes.py write` re-decides every default when a reporting batch
//! enters and writes the answer as `DEPLOYMENT_GENOME`, with the columns it
//! read as `BATCH_COLUMNS`; `the_default_follows_the_batch_rule` re-derives
//! the one from the other here, so the two languages cannot disagree about
//! what ships. A gene the rule removes fails `genes.py check` (and
//! `no_gene_is_due_for_removal` here) until its code is cut.
//!
//! ⭐ ABOVE THE RULE, THE OPERATOR'S TWO LISTS (operator, 2026-08-26): the
//! genes named in `tools/genes.py::OPERATOR_DEFAULT_ON`, generated here as
//! `OPERATOR_DEFAULT_ON` (read by `operator_pins`), ship **on** whatever
//! their batch columns read; the genes named in `OPERATOR_DEFAULT_OFF`
//! (read by `operator_holds`) ship **off** the same way. `batch_rule` still
//! answers for both — the ledger's `rules.batch_decisions` records what the
//! columns alone would have given — so the operator's selection is visible as
//! an override rather than dissolved into the genome. A pin or a hold moves a
//! default only: neither can keep a gene the rule removes from the pool, and
//! no gene is named by both lists. Every default that is neither pinned nor
//! held changes by playing more games.
//!
//! ⭐ WITHIN A FAMILY (operator, 2026-08-23, restated 2026-08-25): every
//! version (`<base>-<n>`) is judged by the rule on its own row, and a family
//! with a version on ships ONE version — its head, the priced version with
//! the highest tracked wins (the ledger's pooled on−off *Diff*, ties to the
//! higher version), when the rule turns the head on, else the best version
//! by tracked wins among those the rule turns on.
//! `tools/genes.py::resolve_family_heads` writes that version into
//! `DEPLOYMENT_GENOME` and records each family's rule-on versions, head and
//! shipped version in `docs/gene_ledger.json` (`rules.family_heads`). A
//! family holds at most three versions; the third-best leaves before a
//! fourth is added (`python3 tools/genes.py versions`).
//!
//! ★★★★ AND IT IS PUBLISHED BESIDE A PRECISION-WEIGHTED POSTERIOR.
//! A threshold in column units is not a threshold in evidence: the screens
//! those columns come from resolve between ±29 and ±101 at 80% power, so the
//! same reading decides differently depending only on which screen priced the
//! gene. `posterior_pp` / `posterior_se_pp` are a random-effects
//! (DerSimonian–Laird) inverse-variance pool of every screen's on−off
//! difference on the win column's scale, with the between-screen
//! disagreement carried in the interval. They are observational evidence,
//! not a deployment rule; `GENE_HEURISTIC_RANKING.md` prints them beside
//! the rule's answer.
//!
//! Verdicts still say what screens proved, but neither verdicts, the sources'
//! win columns, pooled *Diff*, nor posterior values decide what ships: the
//! three batch columns do.
//!
//! The verdict block at the end of `genes.rs` is **generated** by
//! `tools/genes.py` from `gene_screen --analyze --json` outputs and
//! mirrored in `docs/gene_ledger.json`; a test holds the generated file and
//! the JSON together, and another re-derives every `default_on` from the
//! generated batch columns. The verdict rules live in the tool and are
//! repeated here so the reader of either side finds them:
//!
//! - `helps`: win z ≥ 2 with share z > −2, or share
//!   z ≥ 2 with win z > −2 (the screen's own `*` flag; `**` past the
//!   family-wise bar is recorded as strength, not required — with sixty-odd
//!   genes the family-wise bar would leave three on).
//! - `hurts`: the mirror image.
//! - `unresolved`: everything else, including a gene whose two axes
//!   disagree past |z| ≥ 2 and a gene the screens have not measured.
//!
//! The verdict is read off the newest screen that priced the gene; it is
//! evidence only and does not reach the default.
//!
//! `apply_gene_ledger` is what `enable_live_bridge` and
//! `enable_engine_repairs` end with: every live treatment and production
//! treatment the ledger does not default on is withheld, every opt-in it
//! defaults on is enabled. A screenable gene without a measurement row still
//! follows the rule (off until a batch prices it); a Firaxis-only flag, which the screen cannot
//! price, is left exactly as the bundle set it. The `_universe` twins of those
//! two helpers set every flag and skip the ledger: they are the genome's
//! universe, for
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
    /// Whether the batch rule turns the gene on — whether it is in
    /// `DEPLOYMENT_GENOME`, which `the_default_follows_the_batch_rule`
    /// re-derives from `BATCH_COLUMNS`.
    pub default_on: bool,
    /// ± wins per 10,000 on-arm seats at the gene's measured on-rate in the latest
    /// screen that priced it: `GENE_HEURISTIC_RANKING.md`'s
    /// *± Wins / 10k seats*. `None` when no screen has priced it.
    pub wins_last_10k: Option<i32>,
    /// The same figure from the screen before that — *± Wins / 10k seats prior*.
    /// `None` when the gene has only one reading.
    ///
    /// The JSON and `GENE_HEURISTIC_RANKING.md` also carry `wins_third_10k`,
    /// the screen before this one, so a reader can assess trends. These are
    /// the SOURCES' on-arm columns and are evidence only; the default reads
    /// the reporting batches' total-seat columns in `BATCH_COLUMNS`.
    pub wins_prior_10k: Option<i32>,
    /// `GENE_HEURISTIC_RANKING.md`'s *Diff*: the pooled on win rate minus the
    /// pooled off win rate in percentage points, over **every** screen that
    /// priced the gene, each weighted by its on-arm seats. It is evidence only.
    /// `None` when no screen has priced it.
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
    /// The newest screen's paired contrast for this gene.
    pub screen: Option<Measure>,
}

/// The generated policy, deployment genome, batch columns and measurement
/// rows at the end of `genes.rs`, written by `python3 tools/genes.py write`.
mod table {
    pub(super) use super::super::genes::{
        BATCH_COLUMNS, DEPLOYMENT_GENOME, DEPLOYMENT_POLICY, OPERATOR_DEFAULT_OFF,
        OPERATOR_DEFAULT_ON, VERDICTS as ROWS,
    };
}

/// The batch rule reads at most this many batches: the ranking's three
/// columns, newest first.
pub const BATCH_RULE_WINDOW: usize = 3;
/// A gene whose batches are not all positive ships only when their mean
/// exceeds this many wins per 10,000 total seats.
pub const BATCH_RULE_AVERAGE: f64 = 7.0;
/// A gene reading below this in every one of three batches leaves the pool.
pub const BATCH_RULE_REMOVE_BELOW: i32 = -10;

/// What the batch rule says about one gene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchRule {
    /// The gene defaults on.
    On,
    /// The gene defaults off.
    Off,
    /// The gene leaves the gene pool: its code is cut.
    Remove,
}

/// ⭐ THE BATCH RULE — the operator's words (2026-08-25) as a function, the
/// twin of `tools/genes.py::batch_rule`. `columns` are one gene's batch
/// readings newest first: wins ± per 10,000 total seats in the ranking's
/// *Last*, *Prior* and *Third Batch* columns, `None` where that batch did
/// not price the gene. Read over the batches that priced the gene:
///
/// 1. three batches all below [`BATCH_RULE_REMOVE_BELOW`] → `Remove`;
/// 2. two or three batches negative → `Off`;
/// 3. three batches all positive → `On`;
/// 4. three batches, exactly two positive, mean > [`BATCH_RULE_AVERAGE`] → `On`;
/// 5. one or two batches, exactly one positive, mean > 7 → `On`;
/// 6. two batches, both positive → `On`;
/// 7. otherwise `Off` — no batch, or a zero, is neither positive nor negative.
pub fn batch_rule(columns: &[Option<i32>; BATCH_RULE_WINDOW]) -> BatchRule {
    let read: Vec<i32> = columns.iter().flatten().copied().collect();
    if read.is_empty() {
        return BatchRule::Off;
    }
    let positive = read.iter().filter(|&&column| column > 0).count();
    let negative = read.iter().filter(|&&column| column < 0).count();
    let mean = read.iter().sum::<i32>() as f64 / read.len() as f64;
    if read.len() == BATCH_RULE_WINDOW
        && read.iter().all(|&column| column < BATCH_RULE_REMOVE_BELOW)
    {
        return BatchRule::Remove;
    }
    if negative >= 2 {
        return BatchRule::Off;
    }
    if read.len() == BATCH_RULE_WINDOW {
        if positive == BATCH_RULE_WINDOW || (positive == 2 && mean > BATCH_RULE_AVERAGE) {
            return BatchRule::On;
        }
        return BatchRule::Off;
    }
    if (read.len() == 2 && positive == 2) || (positive == 1 && mean > BATCH_RULE_AVERAGE) {
        return BatchRule::On;
    }
    BatchRule::Off
}

/// The three batch columns the rule read for a tag, `None` when no reporting
/// batch has priced it (which the rule reads as off).
pub fn batch_columns(tag: &str) -> Option<&'static [Option<i32>; BATCH_RULE_WINDOW]> {
    table::BATCH_COLUMNS
        .iter()
        .find(|(name, _)| *name == tag)
        .map(|(_, columns)| columns)
}

/// Every gene the ledger has a verdict for, in the generated order.
pub fn gene_ledger() -> &'static [GeneVerdict] {
    table::ROWS
}

/// The explicit deployment policy recorded with this generated set.
pub fn deployment_policy() -> &'static str {
    table::DEPLOYMENT_POLICY
}

/// Whether a tag is in the deployment genome the batch rule and the
/// operator's two lists decided.
pub fn deployment_default_on(tag: &str) -> bool {
    table::DEPLOYMENT_GENOME.contains(&tag)
}

/// ⭐ The genes the operator named on by hand, in generated order. Each ships
/// on whatever [`batch_rule`] reads from its [`batch_columns`].
pub fn operator_pins() -> &'static [&'static str] {
    table::OPERATOR_DEFAULT_ON
}

/// Whether the operator pinned this tag on above the batch rule.
pub fn operator_pinned_on(tag: &str) -> bool {
    table::OPERATOR_DEFAULT_ON.contains(&tag)
}

/// ⭐ The genes the operator named OFF by hand, in generated order — the
/// mirror of [`operator_pins`]. Each ships off whatever [`batch_rule`] reads
/// from its [`batch_columns`].
pub fn operator_holds() -> &'static [&'static str] {
    table::OPERATOR_DEFAULT_OFF
}

/// Whether the operator held this tag off above the batch rule.
pub fn operator_pinned_off(tag: &str) -> bool {
    table::OPERATOR_DEFAULT_OFF.contains(&tag)
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

/// Whether a gene is on in the deployment genome the batch rule and the
/// operator's two lists decided. A screenable tag the operator holds off, or
/// that neither the rule turns on nor the operator pins on, is off, whether
/// or not a batch has priced it. `None` for
/// a gene the screen cannot price (the Firaxis-only flags), which the bundle
/// leaves as it set it.
pub fn ledger_default_on(tag: &str) -> Option<bool> {
    // A host-only flag is never governed by a screen row; the bundle retains
    // control of its live defaults.
    if !screenable(tag) {
        return None;
    }
    Some(deployment_default_on(tag))
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
/// A production opt-in the ledger does not turn on. `--with` may seat one,
/// so a gene priced on the arena can be tried on the live board before the
/// whole-game screen has had its say — the arena is the gate for a tactical
/// gene and the screen is the no-harm check, and until the screen answers
/// there is otherwise no route to the live seat at all except a throwaway
/// build with the constructor flipped.
///
/// It is an *addition*, not the restoration `ledger_held_live_treatment`
/// describes: an opt-in was never in the live universe, so forcing one puts
/// a behaviour on the board that no default would.
pub fn ledger_held_opt_in(tag: &str) -> bool {
    ledger_default_on(tag) != Some(true) && super::gene(tag).is_some_and(|gene| gene.opt_in())
}

/// Every opt-in a live `--with` arm may seat, in registry order.
pub fn ledger_held_opt_ins() -> Vec<&'static str> {
    super::GENES
        .iter()
        .filter(|gene| gene.opt_in())
        .map(|gene| gene.tag)
        .filter(|tag| ledger_held_opt_in(tag))
        .collect()
}

/// Every tag a live `--with` arm may name: a held-off live treatment to
/// restore, or a held-off opt-in to add.
pub fn forceable_treatments() -> Vec<&'static str> {
    let mut tags = ledger_held_live_treatments();
    tags.extend(ledger_held_opt_ins());
    tags
}

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
        let seated = ledger_default_on(gene.tag) == Some(true)
            || (ledger_held_opt_in(gene.tag) && forced_on.contains(&gene.tag));
        if seated && !tags.contains(&gene.tag) {
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
        // may turn on.
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
            } else if ledger_held_opt_in(gene.tag) && forced_on.contains(&gene.tag) {
                // The one route an arena-priced gene has to the live board
                // before the whole-game screen answers. See
                // `ledger_held_opt_in`.
                (gene.enable)(self);
                applied.forced.push(gene.tag);
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
            json["rules"]["deployment_policy"].as_str(),
            Some(table::DEPLOYMENT_POLICY),
            "the JSON ledger and the generated table were written under different policies"
        );
        let genome = json["rules"]["deployment_genome"]
            .as_array()
            .expect("deployment_genome array");
        assert_eq!(genome.len(), table::DEPLOYMENT_GENOME.len());
        for (json_tag, rust_tag) in genome.iter().zip(table::DEPLOYMENT_GENOME) {
            assert_eq!(json_tag.as_str(), Some(*rust_tag));
        }
        let pins = json["rules"]["operator_default_on"]
            .as_array()
            .expect("operator_default_on array");
        assert_eq!(
            pins.len(),
            table::OPERATOR_DEFAULT_ON.len(),
            "the JSON ledger and the generated table hold different operator pins"
        );
        for (json_tag, rust_tag) in pins.iter().zip(table::OPERATOR_DEFAULT_ON) {
            assert_eq!(json_tag.as_str(), Some(*rust_tag));
        }
        let holds = json["rules"]["operator_default_off"]
            .as_array()
            .expect("operator_default_off array");
        assert_eq!(
            holds.len(),
            table::OPERATOR_DEFAULT_OFF.len(),
            "the JSON ledger and the generated table hold different operator holds"
        );
        for (json_tag, rust_tag) in holds.iter().zip(table::OPERATOR_DEFAULT_OFF) {
            assert_eq!(json_tag.as_str(), Some(*rust_tag));
        }
        let columns = json["rules"]["batch_columns"]
            .as_object()
            .expect("batch_columns object");
        assert_eq!(columns.len(), table::BATCH_COLUMNS.len());
        for (tag, rust_columns) in table::BATCH_COLUMNS {
            let json_columns = columns[*tag].as_array().expect("three columns");
            assert_eq!(json_columns.len(), BATCH_RULE_WINDOW, "{tag}");
            for (json_column, rust_column) in json_columns.iter().zip(rust_columns) {
                assert_eq!(
                    json_column.as_i64(),
                    rust_column.map(i64::from),
                    "{tag}: a batch column differs between the table and the JSON"
                );
            }
            let decision = json["rules"]["batch_decisions"][*tag]
                .as_str()
                .expect("decision");
            let expected = match batch_rule(rust_columns) {
                BatchRule::On => "on",
                BatchRule::Off => "off",
                BatchRule::Remove => "remove",
            };
            assert_eq!(
                decision, expected,
                "{tag}: the two languages read the rule differently"
            );
        }
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

    /// The family a tag belongs to: `<base>-<n>` with `n >= 2` and `<base>`
    /// itself a gene is version `n` of `<base>` (`tools/genes.py::families_of`).
    fn family_base(tag: &'static str) -> &'static str {
        match tag.rsplit_once('-') {
            Some((base, version))
                if version.parse::<u32>().is_ok_and(|n| n >= 2)
                    && super::super::gene(base).is_some() =>
            {
                base
            }
            _ => tag,
        }
    }

    /// ★ THE MIRROR: a normal deployment follows the batch rule plus pins;
    /// a reporting-only publication retains the preceding selected genome.
    /// Both forms keep one version per family and agree with every generated
    /// verdict row, so a displayed historical batch cannot silently rewrite
    /// the live defaults.
    #[test]
    fn the_default_follows_the_batch_rule() {
        let retained = deployment_policy() == "operator-retained-selection";
        assert!(
            retained || deployment_policy() == "batch-rule+operator-pins",
            "unknown generated deployment policy: {}",
            deployment_policy()
        );
        let genome = table::DEPLOYMENT_GENOME;
        let mut sorted = genome.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, genome, "the deployment genome is sorted and unique");
        if retained {
            for tag in genome {
                assert!(screenable(tag), "{tag} is retained but not screenable");
                assert_eq!(
                    ledger_default_on(tag),
                    Some(true),
                    "{tag} is retained but not enabled by the runtime ledger"
                );
            }
            let mut per_family: std::collections::BTreeMap<&str, usize> =
                std::collections::BTreeMap::new();
            for tag in genome {
                *per_family.entry(family_base(tag)).or_default() += 1;
            }
            assert!(
                per_family.values().all(|&count| count == 1),
                "a retained deployment has more than one version in a family: {per_family:?}"
            );
        } else {
            let mut rule_on_by_family: std::collections::BTreeMap<&str, Vec<&str>> =
                std::collections::BTreeMap::new();
            for (tag, columns) in table::BATCH_COLUMNS {
                assert!(
                    screenable(tag),
                    "{tag}: only a screenable gene has batch columns"
                );
                let call = batch_rule(columns);
                // ⭐ A pin is on and a hold is off above the rule's answer,
                // which `call` still is.
                let on =
                    (call == BatchRule::On || operator_pinned_on(tag)) && !operator_pinned_off(tag);
                let family = family_base(tag);
                if family == *tag {
                    assert_eq!(
                        genome.contains(tag),
                        on,
                        "{tag}: {columns:?} reads {call:?} (pinned on: {}, held off: {}), but the \
                         generated genome disagrees",
                        operator_pinned_on(tag),
                        operator_pinned_off(tag)
                    );
                } else if on {
                    rule_on_by_family.entry(family).or_default().push(tag);
                }
                if on && family != *tag {
                    continue;
                }
                if family != *tag {
                    assert!(
                        !genome.contains(tag),
                        "{tag}: neither the rule nor a pin turns this version on, yet it ships"
                    );
                }
            }
            for tag in genome {
                assert!(
                    batch_columns(tag).is_some() || operator_pinned_on(tag),
                    "{tag} ships but no reporting batch priced it; the rule turns on only priced genes"
                );
                assert!(
                    batch_columns(tag).map(batch_rule) == Some(BatchRule::On)
                        || operator_pinned_on(tag),
                    "{tag} ships but neither the rule nor an operator pin turns it on"
                );
                assert!(
                    !operator_pinned_off(tag),
                    "{tag} ships but the operator holds it off"
                );
                assert_eq!(
                    ledger_default_on(tag),
                    Some(true),
                    "{tag} is in the genome but not enabled by the runtime ledger"
                );
            }
            for (family, versions) in &rule_on_by_family {
                let shipped: Vec<&str> = versions
                    .iter()
                    .copied()
                    .filter(|v| genome.contains(v))
                    .collect();
                let base_ships = genome.contains(family);
                assert_eq!(
                    shipped.len() + usize::from(base_ships && !versions.contains(family)),
                    1,
                    "family {family}: the rule or a pin turns on {versions:?}; exactly one version \
                     ships"
                );
            }
        }
        let measured_on = gene_ledger().iter().filter(|row| row.default_on).count();
        assert!(
            measured_on <= genome.len(),
            "the measured subset cannot contain more defaults than the genome"
        );
        for row in gene_ledger() {
            assert_eq!(
                row.default_on,
                deployment_default_on(row.tag),
                "{}: the verdict row and the genome disagree",
                row.tag
            );
            assert_eq!(
                ledger_default_on(row.tag),
                screenable(row.tag).then_some(row.default_on),
                "host-only rows stay outside the runtime deployment policy"
            );
        }
    }

    /// ⭐ THE OPERATOR'S PINS: each named gene ships, the rule's own answer for
    /// it is still published, and a pin never rescues a gene the rule removes
    /// from the pool.
    ///
    /// ⚠ What the rule reads for a pinned gene is not an invariant and does
    /// not belong here. This doc used to say all nine were genes the rule read
    /// off at clause 4; that held for the eight hours between the pins landing
    /// (#2536) and the batch published in #2551, which turned seven of the
    /// nine on by itself. `batch_columns` and the ledger's
    /// `rules.batch_decisions` carry today's answer, regenerated with every
    /// batch. A comment cannot.
    #[test]
    fn the_operator_pins_ship_above_the_rule() {
        let retained = deployment_policy() == "operator-retained-selection";
        let pins = operator_pins();
        let mut sorted = pins.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, pins, "the operator's pins are sorted and unique");
        for tag in pins {
            assert!(
                screenable(tag),
                "{tag} is pinned on but the screen cannot price it; a pin is a screenable \
                 gene's tag"
            );
            assert!(
                operator_pinned_on(tag),
                "{tag} is in the pin list but operator_pinned_on says otherwise"
            );
            assert!(
                deployment_default_on(tag),
                "{tag} is pinned on but does not ship"
            );
            assert_eq!(
                ledger_default_on(tag),
                Some(true),
                "{tag} is pinned on but the runtime ledger holds it off"
            );
            if !retained {
                if let Some(columns) = batch_columns(tag) {
                    assert_ne!(
                        batch_rule(columns),
                        BatchRule::Remove,
                        "{tag} is pinned on, but the rule removes it from the gene pool; a pin \
                     decides a default, never whether the code exists"
                    );
                }
            }
        }
        // A pin over a gene no batch has priced ships on its name alone. The
        // operator may well mean exactly that, and on 2026-08-26 did:
        // `barbarian-settler-capture` was asked for by name after the live
        // seat fortified beside a free settler for twenty turns
        // (civvis-20260826T194422Z), and no reporting batch has run since.
        // Each such pin is named here so the genome stays explicable; the
        // next batch prices it like any other gene, and its row below leaves
        // the day a batch column exists.
        const PINNED_BEFORE_PRICING: &[&str] = &["barbarian-settler-capture"];
        for tag in pins {
            assert!(
                batch_columns(tag).is_some() || PINNED_BEFORE_PRICING.contains(tag),
                "{tag} is pinned on before any batch priced it; name it in \
                 PINNED_BEFORE_PRICING with the operator's reason, or wait for a batch"
            );
        }
        for tag in PINNED_BEFORE_PRICING {
            assert!(
                operator_pinned_on(tag),
                "{tag} is named as pinned before pricing but is not a pin"
            );
            assert!(
                batch_columns(tag).is_none(),
                "{tag} is priced now; its PINNED_BEFORE_PRICING row has done its job"
            );
        }
    }

    /// ⭐ THE OPERATOR'S HOLDS: each named gene stays out of the genome
    /// whatever its columns read, the rule's own answer for it is still
    /// published, and no gene is named by both lists.
    #[test]
    fn the_operator_holds_stay_out_of_the_genome() {
        let holds = operator_holds();
        let mut sorted = holds.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, holds, "the operator's holds are sorted and unique");
        for tag in holds {
            assert!(
                screenable(tag),
                "{tag} is held off but the screen cannot price it; a hold is a screenable \
                 gene's tag"
            );
            assert!(
                operator_pinned_off(tag),
                "{tag} is in the hold list but operator_pinned_off says otherwise"
            );
            assert!(
                !operator_pinned_on(tag),
                "{tag} is named both on and off by the operator; a gene belongs to one list"
            );
            assert!(
                !deployment_default_on(tag),
                "{tag} is held off but ships anyway"
            );
            assert_eq!(
                ledger_default_on(tag),
                Some(false),
                "{tag} is held off but the runtime ledger turns it on"
            );
        }
    }

    /// ★ The rule's one action the tool cannot take itself: a gene below −10
    /// in all three batches leaves the gene pool. Until its code is cut this
    /// test — like `genes.py check` — fails and names it.
    #[test]
    fn no_gene_is_due_for_removal() {
        if deployment_policy() == "operator-retained-selection" {
            return;
        }
        let due: Vec<&str> = table::BATCH_COLUMNS
            .iter()
            .filter(|(_, columns)| batch_rule(columns) == BatchRule::Remove)
            .map(|(tag, _)| *tag)
            .collect();
        assert!(
            due.is_empty(),
            "the batch rule removes {due:?} from the gene pool (below {BATCH_RULE_REMOVE_BELOW} \
             in all {BATCH_RULE_WINDOW} batches): cut the gene's row, toggles, field, gated \
             branches, tests and fires json, then `python3 tools/genes.py write`"
        );
    }

    /// The rule, clause by clause, on the operator's own numbers.
    #[test]
    fn the_batch_rule_clause_by_clause() {
        use BatchRule::*;
        let rule = |a: Option<i32>, b: Option<i32>, c: Option<i32>| batch_rule(&[a, b, c]);
        // 1. all three below −10 → remove; −10 itself is not below −10.
        assert_eq!(rule(Some(-11), Some(-30), Some(-12)), Remove);
        assert_eq!(rule(Some(-10), Some(-30), Some(-12)), Off);
        assert_eq!(
            rule(Some(-11), Some(-30), None),
            Off,
            "two batches never remove"
        );
        // 2. two or three negative → off, whatever the third reads.
        assert_eq!(rule(Some(-4), Some(4), Some(-3)), Off);
        assert_eq!(rule(Some(200), Some(-1), Some(-1)), Off);
        assert_eq!(rule(Some(-1), Some(-1), None), Off);
        // 3. three positive → on, however small.
        assert_eq!(rule(Some(1), Some(1), Some(1)), On);
        // 4. exactly two of three positive → on only above a mean of 7.
        assert_eq!(rule(Some(-4), Some(23), Some(15)), On);
        assert_eq!(rule(Some(8), Some(5), Some(-12)), Off);
        assert_eq!(
            rule(Some(-4), Some(13), Some(12)),
            Off,
            "mean 7.0 is not above 7"
        );
        assert_eq!(rule(Some(-5), Some(13), Some(13)), Off, "mean 7.0 exactly");
        assert_eq!(rule(Some(-4), Some(13), Some(13)), On, "mean 7.33");
        assert_eq!(
            rule(Some(0), Some(9), Some(34)),
            On,
            "a zero is not negative"
        );
        assert_eq!(
            rule(Some(0), Some(0), Some(34)),
            Off,
            "one positive of three"
        );
        // 5. one or two batches, exactly one positive → on above a mean of 7.
        assert_eq!(rule(Some(26), None, None), On);
        assert_eq!(rule(Some(7), None, None), Off, "7 is not above 7");
        assert_eq!(rule(Some(16), Some(-1), None), On);
        assert_eq!(rule(Some(-33), Some(7), None), Off);
        assert_eq!(rule(Some(15), Some(0), None), On);
        // 6. two batches both positive → on.
        assert_eq!(rule(Some(16), Some(8), None), On);
        assert_eq!(rule(Some(1), Some(1), None), On);
        // 7. otherwise off: nothing priced, or zeros.
        assert_eq!(rule(None, None, None), Off);
        assert_eq!(rule(Some(0), None, None), Off);
        assert_eq!(rule(Some(0), Some(0), Some(0)), Off);
        assert_eq!(rule(Some(-5), None, None), Off);
    }

    /// Every screenable gene has a default the rule decided, including a gene
    /// no batch has priced yet (off); a Firaxis-only flag has no instrument
    /// and is left alone.
    #[test]
    fn screenable_genes_have_an_explicit_default_and_host_only_flags_are_untouched() {
        assert_eq!(
            ledger_default_on("live-trader-route"),
            None,
            "Firaxis-only: untouched"
        );
        assert!(!screenable("live-trader-route"));
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
    /// holds off, plus the opt-ins the batch rule turns on.
    #[test]
    fn apply_gene_ledger_applies_the_batch_rule_selection_to_live_and_opt_in_genes() {
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
                "{tag}: an opt-in is enabled exactly when the batch rule says on"
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
        // A live (repair) gene the batch rule holds off as of the 2026-08-25
        // batches (+1 / −4 / +5); pick another held live gene if a later batch
        // turns it on (`ledger_held_live_treatments()` lists them).
        let forced = ["whole-turn-backtrack-guard"];
        assert!(ledger_held_live_treatment("whole-turn-backtrack-guard"));
        assert!(
            !ledger_held_live_treatment("parallel-settlers"),
            "host-only treatments already follow their live-universe default"
        );
        assert!(
            !ledger_held_live_treatment("founder-temple"),
            "a withheld production opt-in is not a live-universe override"
        );
        // A held-off opt-in is not a live-universe override either, but it
        // IS a treatment an arm may *add*: the two predicates ask different
        // questions and `forceable_treatments` is their sum. Taken from the
        // registry rather than named, because which opt-ins the ledger holds
        // off changes with every batch.
        let held_opt_in = ledger_held_opt_ins()
            .first()
            .copied()
            .expect("the registry holds opt-ins the ledger has not turned on");
        assert!(!ledger_held_live_treatment(held_opt_in));
        assert!(!ledger_held_opt_in("whole-turn-backtrack-guard"));
        let forceable = forceable_treatments();
        assert!(forceable.contains(&"whole-turn-backtrack-guard"));
        assert!(forceable.contains(&held_opt_in));
        let seated = deployment_treatments_with_forced_live(&[held_opt_in]);
        assert!(
            seated.contains(&held_opt_in) && !deployment_treatments().contains(&held_opt_in),
            "an arm may seat a held-off opt-in without moving the deployment genome"
        );
        let mut seat = super::AdvancedAi::new();
        seat.enable_live_bridge_universe();
        let applied = seat.apply_gene_ledger_with_forced_live(&[held_opt_in]);
        assert!(applied.forced.contains(&held_opt_in));
        assert!(ledger_held_live_treatments().contains(&"whole-turn-backtrack-guard"));

        let deployed = deployment_treatments();
        let forced_deployment = deployment_treatments_with_forced_live(&forced);
        assert!(
            !deployed.contains(&"whole-turn-backtrack-guard"),
            "the verification override must not change deployment"
        );
        assert!(
            forced_deployment.contains(&"whole-turn-backtrack-guard"),
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
        assert!(
            ai.base.whole_turn_backtrack_guard,
            "the named live treatment stands"
        );
        assert!(
            !ai.blind_objective_strength,
            "another ledger-held treatment stays off unless named too"
        );
        assert_eq!(applied.forced, vec!["whole-turn-backtrack-guard"]);
        assert!(
            !applied.withheld.contains(&"whole-turn-backtrack-guard"),
            "an explicit arm cannot report its restored gene as withheld"
        );
    }
}
