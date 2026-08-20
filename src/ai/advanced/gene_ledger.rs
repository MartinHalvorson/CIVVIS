//! The gene ledger: what the screens have measured about every gene, and the
//! deployment genome that follows from it.
//!
//! ★★★★ THE DEFAULTS ARE THE BEST GENOME, AND THE BEST GENOME IS DATA.
//! Operator directive 2026-08-20: the defaults for the genes reflect our best
//! genome — only genes that provably help are on; unhelpful genes default
//! off — so every verification game (the live seat, the `live` and
//! `advanced_synergy` arms, the ladder) plays the genome the measurements
//! support, while the screens keep testing and trying to improve the less
//! helpful genes. Until this module, "on by default" meant "somebody wrote
//! `self.enable_x()` into the bundle", and the phase-1 anchors measured the
//! all-on bundle at 7.5% wins against 27% for all-off.
//!
//! The table in `gene_ledger_table.rs` is **generated** by
//! `tools/gene_ledger.py` from `gene_screen --analyze --json` outputs and
//! mirrored in `docs/gene_ledger.json`; a test holds the generated file and
//! the JSON together. The verdict rules live in the tool and are repeated
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
//! The **native** (all six lanes) regime governs the default; the **war**
//! regime (`domination,score`) is recorded beside it, and a gene that helps
//! at war and is unresolved natively is on (it has a regime where it provably
//! helps and none where it provably hurts). A gene that hurts natively is
//! off whatever the war regime says, because the verification games are the
//! all-six regime.
//!
//! `apply_gene_ledger` is what `enable_live_bridge` and
//! `enable_engine_repairs` end with: every live treatment and production
//! treatment whose verdict is not `helps` is withheld, every opt-in whose
//! verdict is `helps` is enabled, and a gene with no ledger row (the
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
    /// Past the family-wise bar in the regime that decided the verdict.
    pub family_wise: bool,
    pub native: Option<Measure>,
    pub war: Option<Measure>,
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

/// Whether a gene is on in the deployment genome: `helps` on, anything else
/// off — including a screenable gene no screen has measured yet, which is
/// not proven either. `None` for a gene no native screen can price (the
/// Firaxis-only flags), which the bundle leaves as it set it.
pub fn ledger_default_on(tag: &str) -> Option<bool> {
    match ledger_verdict(tag) {
        Some(row) => Some(row.verdict == Verdict::Helps),
        None if screenable(tag) => Some(false),
        None => None,
    }
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
                row.verdict == Verdict::Helps,
                entry["default_on"].as_bool().expect("default_on"),
                "{tag}: default differs between the table and the JSON"
            );
        }
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
        let forced = ["stacked-escort"];
        assert!(ledger_held_live_treatment("stacked-escort"));
        assert!(
            !ledger_held_live_treatment("parallel-settlers"),
            "host-only treatments already follow their live-universe default"
        );
        assert!(
            !ledger_held_live_treatment("strategic-wonders"),
            "a production treatment is not a live-universe override"
        );
        assert!(ledger_held_live_treatments().contains(&"stacked-escort"));

        let deployed = deployment_treatments();
        let forced_deployment = deployment_treatments_with_forced_live(&forced);
        assert!(
            !deployed.contains(&"stacked-escort"),
            "the verification override must not change deployment"
        );
        assert!(
            forced_deployment.contains(&"stacked-escort"),
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
        assert!(ai.stacked_escort, "the named live treatment stands");
        assert!(
            !ai.settler_stack_discipline(),
            "neighbouring ledger-held treatments stay off unless named too"
        );
        assert_eq!(applied.forced, vec!["stacked-escort"]);
        assert!(
            !applied.withheld.contains(&"stacked-escort"),
            "an explicit arm cannot report its restored gene as withheld"
        );
    }
}
