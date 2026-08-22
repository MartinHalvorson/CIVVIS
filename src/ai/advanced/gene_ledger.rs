//! The gene ledger: what the screens have measured about every gene, and the
//! deployment genome that follows from it.
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
//! last and prior native win columns are positive, or when their average
//! clears +15 with neither column below −10; every other gene defaults off.
//! A win column is wins added per 10,000 games at the gene's measured on-rate
//! in one native screen — `(win_on − 1/players) × 10,000`, against the 1-in-
//! `players` a seat wins by chance — and it is the same number
//! `HEURISTIC_GENE_RANKING.md` prints. A gene the screens have read fewer
//! than twice has no prior column to agree with it, so it is off: one screen
//! is never a result. The war regime does not enter the default.
//!
//! What that replaced: the default used to be `verdict == helps`, one
//! screen's significance test on the deciding regime. The verdicts are still
//! recorded and still say what the screens proved; they no longer decide what
//! ships. A gene can now be `helps` and off (its prior reading was against
//! it) or `hurts` and on (two positive win columns since).
//!
//! The table in `gene_ledger_table.rs` is **generated** by
//! `tools/gene_ledger.py` from `gene_screen --analyze --json` outputs and
//! mirrored in `docs/gene_ledger.json`; a test holds the generated file and
//! the JSON together, and another re-derives every `default_on` from the two
//! columns beside it. The verdict rules live in the tool and are repeated
//! here so the reader of either side finds them:
//!
//! - `helps`: in a regime of record, win z ≥ 2 with share z > −2, or share
//!   z ≥ 2 with win z > −2 (the screen's own `*` flag; `**` past the
//!   family-wise bar is recorded as strength, not required — with sixty-odd
//!   genes the family-wise bar would leave three on).
//! - `hurts`: the mirror image.
//! - `unresolved`: everything else, including a gene whose two axes
//!   disagree past |z| ≥ 2 and a gene the screens have not measured.
//!
//! The **native** (all six lanes) regime governs the verdict; the **war**
//! regime (`domination,score`) is recorded beside it, and a gene unresolved
//! natively takes the war regime's verdict when that resolves. Neither the
//! war regime nor the verdict reaches the default, which is native-only by
//! construction: the verification games are the all-six regime.
//!
//! `apply_gene_ledger` is what `enable_live_bridge` and
//! `enable_engine_repairs` end with: every live treatment and production
//! treatment the ledger does not default on is withheld, every opt-in it
//! defaults on is enabled, and a gene with no ledger row (the
//! Firaxis-only flags, which no native screen can price) is left exactly as
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

/// One regime's measurement of one gene, as the screen printed it.
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
    /// in the module header, decided by `tools/gene_ledger.py` and checked
    /// here against the two columns below by `the_default_follows_the_win_columns`.
    pub default_on: bool,
    /// ± wins per 10,000 games at the gene's measured on-rate in the latest
    /// native screen that priced it: `HEURISTIC_GENE_RANKING.md`'s
    /// *± Wins Last 10k*. `None` when no native screen has priced it.
    pub wins_last_10k: Option<i32>,
    /// The same figure from the native screen before that — *± Wins 10k
    /// Prior*. `None` when the gene has only one native reading.
    pub wins_prior_10k: Option<i32>,
    /// Past the family-wise bar in the regime that decided the verdict.
    pub family_wise: bool,
    pub native: Option<Measure>,
    pub war: Option<Measure>,
}

/// A gene's win columns clear the deployment rule: both positive, or an
/// average above `AVERAGE_BAR` with neither column below `COLUMN_FLOOR`.
/// Fewer than two native readings is off — the mirror of
/// `tools/gene_ledger.py`'s `default_from_win_columns`, so a hand-edited
/// table cannot quietly ship a gene the rule does not.
pub fn win_columns_default_on(last: Option<i32>, prior: Option<i32>) -> bool {
    /// Wins per ten thousand games the two-column average must clear.
    const AVERAGE_BAR: f64 = 15.0;
    /// No column may sit below this, however good the other one is.
    const COLUMN_FLOOR: i32 = -10;
    let (Some(last), Some(prior)) = (last, prior) else {
        return false;
    };
    if last > 0 && prior > 0 {
        return true;
    }
    f64::from(last + prior) / 2.0 > AVERAGE_BAR && last >= COLUMN_FLOOR && prior >= COLUMN_FLOOR
}

#[path = "gene_ledger_table.rs"]
mod table;

/// Every gene the ledger has a verdict for, in the generated order.
pub fn gene_ledger() -> &'static [GeneVerdict] {
    table::ROWS
}

/// The ledger's row for a published tag, if the screens have measured it.
pub fn ledger_verdict(tag: &str) -> Option<&'static GeneVerdict> {
    table::ROWS.iter().find(|row| row.tag == tag)
}

/// Whether a native screen can price a tag at all: the engine repairs, the
/// production treatments and the opt-ins — `gene_screen`'s own universe. A
/// Firaxis-only flag is not here, and the ledger has nothing to say about it.
pub fn screenable(tag: &str) -> bool {
    crate::elo::ENGINE_REPAIR_TREATMENTS.contains(&tag)
        || super::PRODUCTION_TREATMENTS
            .iter()
            .chain(super::PRODUCTION_OPT_INS)
            .any(|(_, row_tag, _)| *row_tag == tag)
}

/// Whether a gene is on in the deployment genome: the ledger's own
/// `default_on`, which is the win-column rule in the module header — so a
/// screenable gene no screen has measured, or has measured only once, is off.
/// `None` for a gene no native screen can price (the Firaxis-only flags),
/// which the bundle leaves as it set it.
pub fn ledger_default_on(tag: &str) -> Option<bool> {
    // A host-only flag is never governed by a native row — even when one
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
    ledger_default_on(tag) == Some(false)
        && super::LIVE_TREATMENTS
            .iter()
            .any(|(_, live_tag, _)| *live_tag == tag)
}

/// Every live treatment an explicit ledger-override arm may restore, in
/// registry order. This is deliberately narrower than every default-off gene:
/// the caller begins from the live universe, so only its withheld rows can be
/// restored without silently enabling a different bundle.
pub fn ledger_held_live_treatments() -> Vec<&'static str> {
    super::LIVE_TREATMENTS
        .iter()
        .map(|(_, tag, _)| *tag)
        .filter(|tag| ledger_held_live_treatment(tag))
        .collect()
}

/// The treatments a live arm actually plays: every live treatment the ledger
/// does not hold off, plus any explicitly forced ledger-held live treatment,
/// and every opt-in the ledger turns on. This is what the live seat's `genome`
/// event reports — the list that used to be `LIVE_BRIDGE_TREATMENTS` whole,
/// which the ledger makes untrue.
pub fn deployment_treatments_with_forced_live(forced_on: &[&str]) -> Vec<&'static str> {
    let mut tags: Vec<&'static str> = super::LIVE_TREATMENTS
        .iter()
        .map(|(_, tag, _)| *tag)
        .filter(|tag| {
            ledger_default_on(tag) != Some(false)
                || (ledger_held_live_treatment(tag) && forced_on.contains(tag))
        })
        .collect();
    tags.extend(
        super::PRODUCTION_OPT_INS
            .iter()
            .map(|(_, tag, _)| *tag)
            .filter(|tag| ledger_default_on(tag) == Some(true)),
    );
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
        for &(_, tag, disable) in super::LIVE_TREATMENTS
            .iter()
            .chain(super::PRODUCTION_TREATMENTS)
        {
            if ledger_held_live_treatment(tag) && forced_on.contains(&tag) {
                applied.forced.push(tag);
            } else if ledger_default_on(tag) == Some(false) {
                disable(self);
                applied.withheld.push(tag);
            }
        }
        for &(_, tag, enable) in super::PRODUCTION_OPT_INS {
            if ledger_default_on(tag) == Some(true) {
                enable(self);
                applied.enabled.push(tag);
            }
        }
        applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated table and `docs/gene_ledger.json` are two writings of
    /// one measurement; `tools/gene_ledger.py --write` produces both.
    #[test]
    fn the_generated_table_matches_the_json_ledger() {
        let json: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/gene_ledger.json"))
                .expect("docs/gene_ledger.json parses");
        let genes = json["genes"].as_array().expect("genes array");
        assert_eq!(
            genes.len(),
            gene_ledger().len(),
            "the Rust table and the JSON ledger hold different gene counts; run \
             `python3 tools/gene_ledger.py --write`"
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
        }
    }

    /// The rule itself: every default in the generated table is the one the
    /// two win columns beside it produce. The tool decides; this re-derives.
    #[test]
    fn the_default_follows_the_win_columns() {
        let mut on = 0;
        for row in gene_ledger() {
            assert_eq!(
                row.default_on,
                win_columns_default_on(row.wins_last_10k, row.wins_prior_10k),
                "{}: default {} does not follow its win columns {:?}/{:?}",
                row.tag,
                row.default_on,
                row.wins_last_10k,
                row.wins_prior_10k
            );
            on += usize::from(row.default_on);
        }
        assert!(on > 0, "a genome with no gene on is a broken regeneration");
    }

    /// The rule's three clauses, at their boundaries.
    #[test]
    fn the_win_column_rule_reads_both_columns() {
        assert!(win_columns_default_on(Some(1), Some(1)), "both positive");
        assert!(!win_columns_default_on(Some(1), Some(0)), "zero is not positive");
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
            !win_columns_default_on(Some(81), None),
            "one native reading has nothing to agree with it"
        );
        assert!(!win_columns_default_on(None, None), "unmeasured is off");
    }

    /// A screenable gene the screens have not measured is not proven, so it
    /// is off; a Firaxis-only flag has no native instrument and is left alone.
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
            "a host-only flag is never governed by a native row"
        );
        for repair in crate::elo::ENGINE_REPAIR_TREATMENTS {
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
        let known: Vec<&str> = super::super::LIVE_TREATMENTS
            .iter()
            .chain(super::super::PRODUCTION_TREATMENTS)
            .chain(super::super::PRODUCTION_OPT_INS)
            .map(|(_, tag, _)| *tag)
            .collect();
        for row in gene_ledger() {
            assert!(
                known.contains(&row.tag),
                "ledger row {} names no live treatment, production treatment or opt-in",
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
        for &(_, tag, _) in super::super::LIVE_TREATMENTS
            .iter()
            .chain(super::super::PRODUCTION_TREATMENTS)
        {
            match ledger_default_on(tag) {
                Some(false) => assert!(applied.withheld.contains(&tag), "{tag} should be withheld"),
                _ => assert!(!applied.withheld.contains(&tag), "{tag} should stand"),
            }
        }
        for &(_, tag, _) in super::super::PRODUCTION_OPT_INS {
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
        for tag in crate::elo::LIVE_BRIDGE_TREATMENTS {
            assert_eq!(
                deployed.contains(tag),
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
            !ai.war_economy,
            "neighbouring ledger-held treatments stay off unless named too"
        );
        assert_eq!(applied.forced, vec!["governor-every-lane"]);
        assert!(
            !applied.withheld.contains(&"governor-every-lane"),
            "an explicit arm cannot report its restored gene as withheld"
        );
    }
}
