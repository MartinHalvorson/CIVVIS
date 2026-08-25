//! The gene ledger: what the screen has measured about every gene, beside the
//! explicit deployment genome the operator selected.
//!
//! ⭐ ONE SCREEN. Operator directive 2026-08-22: every row here comes from the
//! same shape — six majors on 74x46 continents with nine city-states, Online
//! speed to its own 250-turn clock, all six victory lanes, every seat carrying
//! its own drawn genome against the best-genome baseline. There is no second
//! regime to reconcile, and `tools/genes.py` refuses a source played at
//! any other profile. The columns carried over from the pre-2026-08-22 Pangaea
//! screens are marked `legacy` in `docs/gene_ledger.json`: history retained
//! as evidence beside the pinned deployment selection.
//!
//! ★★★★ THE DEFAULT IS AN EXPLICIT OPERATOR-PINNED GENOME.
//! Operator directive 2026-08-24: preserve the 36 selections then deployed
//! and explicitly promote `unit-cost-efficiency`, `unit-objective-memory`,
//! `camp-party`, `slot-kind-tiebreak`, `promote-when-wounded`,
//! `religion-sues-peace`, `lane-great-people`, `one-launch-pad`, and
//! `civilian-rescue`; then `missionary-evades-raiders`, `district-planning`,
//! `missionary-last-charge-explores`, `settlement-gap-target`,
//! `religious-defence-scales`, `lane-policy-deck`, and
//! `science-multiplier-payoff`, for 52 enabled genes; then (2026-08-24,
//! "default this gene to true initially once you write and merge it")
//! `science-victory-drive`, pinned on before its first screen, for 53.
//! `DEPLOYMENT_GENOME` is that exact list. A screen refresh updates evidence,
//! not the runtime default; changing a default requires an explicit operator
//! update to the pinned list and regeneration.
//!
//! ★★★★ AND IT IS PUBLISHED BESIDE A PRECISION-WEIGHTED POSTERIOR.
//! A threshold in column units is not a threshold in evidence: the screens
//! those columns come from resolve between ±29 and ±101 at 80% power, so the
//! same reading decides differently depending only on which screen priced the
//! gene. `posterior_pp` / `posterior_se_pp` are a random-effects
//! (DerSimonian–Laird) inverse-variance pool of every screen's on−off
//! difference on the win column's scale, with the between-screen
//! disagreement carried in the interval. They are observational evidence,
//! not a deployment rule; `HEURISTIC_GENE_RANKING.md` prints the evidence for
//! future explicit selections.
//!
//! Verdicts still say what screens proved, but neither verdicts, win columns,
//! pooled *Diff*, nor posterior values mechanically decide what ships.
//!
//! The verdict block at the end of `genes.rs` is **generated** by
//! `tools/genes.py` from `gene_screen --analyze --json` outputs and
//! mirrored in `docs/gene_ledger.json`; a test holds the generated file and
//! the JSON together, and another validates every `default_on` against the
//! generated pinned list. The verdict rules live in the tool and are repeated
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
//! The verdict is read off the newest screen that priced the gene; it is
//! evidence only and does not reach the default.
//!
//! `apply_gene_ledger` is what `enable_live_bridge` and
//! `enable_engine_repairs` end with: every live treatment and production
//! treatment the ledger does not default on is withheld, every opt-in it
//! defaults on is enabled. A screenable gene without a measurement row still
//! follows the pinned list; a Firaxis-only flag, which the screen cannot
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
    /// Whether the gene is on in the explicit operator-pinned deployment
    /// genome, checked here against `DEPLOYMENT_GENOME`.
    pub default_on: bool,
    /// ± wins per 10,000 on-arm seats at the gene's measured on-rate in the latest
    /// screen that priced it: `HEURISTIC_GENE_RANKING.md`'s
    /// *± Wins / 10k seats*. `None` when no screen has priced it.
    pub wins_last_10k: Option<i32>,
    /// The same figure from the screen before that — *± Wins / 10k seats prior*.
    /// `None` when the gene has only one reading.
    ///
    /// The JSON and `HEURISTIC_GENE_RANKING.md` also carry `wins_third_10k`,
    /// the screen before this one, so a reader can assess trends. All windows
    /// are evidence only; none selects a deployment default automatically.
    pub wins_prior_10k: Option<i32>,
    /// `HEURISTIC_GENE_RANKING.md`'s *Diff*: the pooled on win rate minus the
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

/// The generated policy, pinned list, and measurement rows at the end of
/// `genes.rs`, written by `python3 tools/genes.py write`.
mod table {
    pub(super) use super::super::genes::{DEPLOYMENT_GENOME, DEPLOYMENT_POLICY, VERDICTS as ROWS};
}

/// Every gene the ledger has a verdict for, in the generated order.
pub fn gene_ledger() -> &'static [GeneVerdict] {
    table::ROWS
}

/// The explicit deployment policy recorded with this generated set.
pub fn deployment_policy() -> &'static str {
    table::DEPLOYMENT_POLICY
}

/// Whether a tag is in the explicit operator-pinned deployment genome.
pub fn operator_default_on(tag: &str) -> bool {
    table::DEPLOYMENT_GENOME.contains(&tag)
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

/// Whether a gene is on in the explicit deployment genome. A screenable tag
/// outside the pinned list is off, whether or not a screen has measured it.
/// `None` for a gene the screen cannot price (the Firaxis-only flags),
/// which the bundle leaves as it set it.
pub fn ledger_default_on(tag: &str) -> Option<bool> {
    // A host-only flag is never governed by a screen row — even when one
    // exists: such a row measured a native stand-in that no longer runs
    // (`step-and-reassess`, 2026-08-21) and must not govern the bridge.
    if !screenable(tag) {
        return None;
    }
    Some(operator_default_on(tag))
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
            json["rules"]["deployment_policy"].as_str(),
            Some(table::DEPLOYMENT_POLICY),
            "the JSON ledger and the generated table were written under different policies"
        );
        let pinned = json["rules"]["deployment_genome"]
            .as_array()
            .expect("deployment_genome array");
        assert_eq!(pinned.len(), table::DEPLOYMENT_GENOME.len());
        for (json_tag, rust_tag) in pinned.iter().zip(table::DEPLOYMENT_GENOME) {
            assert_eq!(json_tag.as_str(), Some(*rust_tag));
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

    /// Every generated row agrees with the explicit list, so observations
    /// cannot quietly re-decide a runtime default.
    #[test]
    fn the_default_matches_the_operator_pinned_genome() {
        assert_eq!(deployment_policy(), "operator-pinned");
        let mut measured_on = 0;
        for row in gene_ledger() {
            assert_eq!(
                row.default_on,
                operator_default_on(row.tag),
                "{} differs from the generated operator-pinned list",
                row.tag
            );
            assert_eq!(
                ledger_default_on(row.tag),
                screenable(row.tag).then_some(row.default_on),
                "host-only rows stay outside the runtime deployment policy"
            );
            measured_on += usize::from(row.default_on);
        }
        assert_eq!(
            table::DEPLOYMENT_GENOME.len(),
            53,
            "an operator selection changed; update it deliberately"
        );
        assert!(
            measured_on <= table::DEPLOYMENT_GENOME.len(),
            "the measured subset cannot contain more defaults than the pinned genome"
        );
        for tag in table::DEPLOYMENT_GENOME {
            assert_eq!(
                ledger_default_on(tag),
                Some(true),
                "{tag} is pinned but not enabled by the runtime ledger"
            );
        }
    }

    #[test]
    fn the_sixteen_explicit_promotions_are_pinned_on() {
        for tag in [
            "unit-cost-efficiency",
            "unit-objective-memory",
            "camp-party",
            "slot-kind-tiebreak",
            "promote-when-wounded",
            "religion-sues-peace",
            "lane-great-people",
            "one-launch-pad",
            "civilian-rescue",
            "missionary-evades-raiders",
            "district-planning",
            "missionary-last-charge-explores",
            "settlement-gap-target",
            "religious-defence-scales",
            "lane-policy-deck",
            "science-multiplier-payoff",
        ] {
            assert!(operator_default_on(tag), "{tag} was not pinned on");
        }
    }

    /// Every screenable gene has an explicit pinned state, including a gene
    /// whose first measurement has not landed; a Firaxis-only flag has no
    /// instrument and is left alone.
    #[test]
    fn screenable_genes_have_an_explicit_default_and_host_only_flags_are_untouched() {
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
    /// holds off, plus the opt-ins it turns on from the explicit pinned list.
    #[test]
    fn apply_gene_ledger_applies_the_pinned_selection_to_live_and_opt_in_genes() {
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
                "{tag}: an opt-in is enabled exactly when the pinned ledger says on"
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
        let forced = ["settler-guard-holds"];
        assert!(ledger_held_live_treatment("settler-guard-holds"));
        assert!(
            !ledger_held_live_treatment("parallel-settlers"),
            "host-only treatments already follow their live-universe default"
        );
        assert!(
            !ledger_held_live_treatment("founder-temple"),
            "a withheld production opt-in is not a live-universe override"
        );
        assert!(ledger_held_live_treatments().contains(&"settler-guard-holds"));

        let deployed = deployment_treatments();
        let forced_deployment = deployment_treatments_with_forced_live(&forced);
        assert!(
            !deployed.contains(&"settler-guard-holds"),
            "the verification override must not change deployment"
        );
        assert!(
            forced_deployment.contains(&"settler-guard-holds"),
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
        assert!(ai.settler_guard_holds, "the named live treatment stands");
        assert!(
            !ai.blind_objective_strength,
            "another ledger-held treatment stays off unless named too"
        );
        assert_eq!(applied.forced, vec!["settler-guard-holds"]);
        assert!(
            !applied.withheld.contains(&"settler-guard-holds"),
            "an explicit arm cannot report its restored gene as withheld"
        );
    }
}
