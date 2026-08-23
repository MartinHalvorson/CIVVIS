//! Elo arithmetic and the built-in agent registry.
//!
//! The Elo tournament harness, its persistent ledgers (`data/elo_ratings*`),
//! and the strategy league were retired on 2026-08-23 (#2357): the gene
//! screen prices behaviours, and nothing rates named agents against each
//! other for now. What stays is what the rest of the program still uses —
//! the Elo expectation (`expected`, `win_shares`, which the win-odds
//! annotation is built on), the built-in agents (`BUILTIN_AIS`,
//! `builtin_ai`, `builtin_send_ai`), and the provenance report that keeps a
//! degraded artifact from playing under a trained name. The frozen anchor's
//! behaviour pin lives on in `main.rs` (`ANCHOR_BEHAVIOUR_FNV`), and
//! `docs/ELO_REPINS.md` remains its paper trail. Bringing a rating system
//! back for finished genomes is planned; see docs/ROADMAP.md.
use crate::ai::{AdvancedAi, Ai, BasicAi, RandomAi};

pub const BUILTIN_AIS: &[&str] = &[
    "advanced",
    "advanced_evolved",
    "advanced_v1",
    "basic",
    "random",
    "evolved",
    "strategic",
    "strategic_deep",
];

// ⭐ The gene registry — every live, host-only, repair, production and opt-in
// gene — is `crate::ai::advanced::genes`; the five tag lists that used to
// live here are columns of its rows (`live_tags()`, `host_only_tags()`,
// `repair_tags()`, `repair_tags_on(axis)`).

pub fn expected(ra: f64, rb: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf((rb - ra) / 400.0))
}

/// Each rating's chance of *winning outright* against the rest of the table,
/// summing to 1.
pub fn win_shares(ratings: &[f64]) -> Vec<f64> {
    let top = ratings.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let weights: Vec<f64> = ratings
        .iter()
        .map(|rating| 10f64.powf((rating - top) / 400.0))
        .collect();
    let total: f64 = weights.iter().sum();
    if total <= 0.0 || !total.is_finite() {
        return vec![1.0 / ratings.len().max(1) as f64; ratings.len()];
    }
    weights.iter().map(|weight| weight / total).collect()
}

/// Directory `builtin_ai` resolves trained artifacts from.
pub const ARTIFACT_DIR: &str = "evolved";
/// Evolved strategy genome written by `civvis evolve`.
pub const CHAMPION_FILE: &str = "best.json";
/// Distilled scalar value net written by `tools/train_valuenet.py`.
pub const VALUENET_FILE: &str = "valuenet.json";

/// Build a named built-in agent (`BUILTIN_AIS`), resolving trained artifacts
/// from `ARTIFACT_DIR` and falling back to the scripted controller when one is
/// missing — `evolved` without a champion is `advanced`, `strategic` without a
/// value net is the score-only search. A name that is not a built-in plays as
/// `basic`, as it always has at game start.
///
/// ⚠ The 228 evaluator arms that used to be constructed here — `advanced_<x>`,
/// `advanced_without_<x>`, `live_without_<x>`, `live_target_<lane>` — are gone
/// (2026-08-23). One flag per arm, priced against a fixed background, was the
/// instrument the gene screen replaced: `gene_screen` prices every gene from
/// every seat of a random-genome batch, and the live seat withholds a shipped
/// behaviour through `civvis_orders --without`. See `docs/GENE_SCREEN.md`.
pub fn builtin_ai(name: &str, seed: u64) -> Box<dyn Ai> {
    let champion = || crate::evolve::load_champion(ARTIFACT_DIR);
    match name {
        "advanced" => Box::new(AdvancedAi::new()),
        "advanced_v1" => Box::new(AdvancedAi::legacy()),
        "advanced_evolved" | "evolved" => {
            Box::new(champion().map(AdvancedAi::with_weights).unwrap_or_default())
        }
        "basic" => Box::new(BasicAi::new()),
        "random" => Box::new(RandomAi::new(seed)),
        "strategic" => {
            let weights = champion().unwrap_or_default();
            if crate::valuenet::ValueNet::load_width(ARTIFACT_DIR, crate::evolve::FEATURE_WIDTH)
                .is_some()
            {
                Box::new(crate::strategic::StrategicAi::with_weights(weights))
            } else {
                Box::new(crate::strategic::StrategicAi::score_only_with_weights(
                    weights,
                ))
            }
        }
        "strategic_deep" => {
            let mut ai =
                crate::strategic::StrategicAi::with_weights(champion().unwrap_or_default());
            ai.review_every = 20;
            ai.horizon = 80;
            Box::new(ai)
        }
        _ => Box::new(BasicAi::new()),
    }
}

/// `builtin_ai` for a seat that has to cross a thread: the server's session
/// lives behind a lock, so its agents are `Box<dyn Ai + Send>`.
pub fn builtin_send_ai(name: &str, seed: u64) -> Box<dyn Ai + Send> {
    let champion = || crate::evolve::load_champion(ARTIFACT_DIR);
    match name {
        "advanced" => Box::new(AdvancedAi::new()),
        "advanced_v1" => Box::new(AdvancedAi::legacy()),
        "advanced_evolved" | "evolved" => {
            Box::new(champion().map(AdvancedAi::with_weights).unwrap_or_default())
        }
        "basic" => Box::new(BasicAi::new()),
        "random" => Box::new(RandomAi::new(seed)),
        "strategic" => Box::new(crate::strategic::StrategicAi::with_weights(
            champion().unwrap_or_default(),
        )),
        _ => Box::new(AdvancedAi::new()),
    }
}

/// One trained artifact a builtin name reads, and whether it loaded.
///
/// `definitional` separates the two ways a name depends on an artifact. A
/// definitional artifact *is* the agent: without it `builtin_ai` returns a
/// different agent under the same name. A non-definitional one only tunes
/// the agent, so its absence leaves the name honest but the numbers
/// untrained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactStatus {
    pub file: &'static str,
    pub found: bool,
    pub definitional: bool,
}

/// What a built-in name actually loads and plays as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProvenance {
    /// The name the caller asked for.
    pub requested: String,
    /// Every artifact the name reads, in the order it reads them.
    pub artifacts: Vec<ArtifactStatus>,
    /// Canonical identity of the agent that actually plays. Equals
    /// `requested` unless a definitional artifact is missing.
    pub effective: &'static str,
}

impl AgentProvenance {
    /// True when the name promises more than the loaded artifacts deliver.
    pub fn degraded(&self) -> bool {
        self.artifacts
            .iter()
            .any(|artifact| artifact.definitional && !artifact.found)
    }

    /// True when some artifact the name reads did not load, whether or not
    /// that changed which agent plays.
    pub fn untrained(&self) -> bool {
        self.artifacts.iter().any(|artifact| !artifact.found)
    }

    pub fn missing(&self) -> Vec<&'static str> {
        self.artifacts
            .iter()
            .filter(|artifact| !artifact.found)
            .map(|artifact| artifact.file)
            .collect()
    }

    /// One reportable line, e.g.
    /// `evolved: plays as advanced (missing best.json)`.
    pub fn line(&self) -> String {
        let missing = self.missing();
        if missing.is_empty() {
            return if self.artifacts.is_empty() {
                if self.effective == self.requested {
                    format!("{}: scripted, no artifacts required", self.requested)
                } else {
                    format!(
                        "{}: plays as {} (scripted, no artifacts required)",
                        self.requested, self.effective
                    )
                }
            } else if self.effective != self.requested {
                format!(
                    "{}: plays as {} (loaded {})",
                    self.requested,
                    self.effective,
                    self.artifacts_list()
                )
            } else {
                format!("{}: loaded {}", self.requested, self.artifacts_list())
            };
        }
        let plays = match self.degraded() {
            true => format!("plays as {}", self.effective),
            false => format!("plays as {} with untrained defaults", self.requested),
        };
        format!(
            "{}: {} (missing {})",
            self.requested,
            plays,
            missing.join(", ")
        )
    }

    fn artifacts_list(&self) -> String {
        self.artifacts
            .iter()
            .map(|artifact| artifact.file)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// What `builtin_ai(name, _)` will actually construct from the artifacts
/// under `dir`, so a tournament can refuse to rate a name that would silently
/// play as a different controller.
pub fn builtin_provenance(name: &str, dir: &str) -> AgentProvenance {
    let champion = crate::evolve::load_champion(dir).is_some();
    let net = crate::valuenet::ValueNet::load_width(dir, crate::evolve::FEATURE_WIDTH).is_some();
    let genome = |definitional| ArtifactStatus {
        file: CHAMPION_FILE,
        found: champion,
        definitional,
    };
    let value = |definitional| ArtifactStatus {
        file: VALUENET_FILE,
        found: net,
        definitional,
    };
    let (artifacts, effective): (Vec<ArtifactStatus>, &'static str) = match name {
        // The genome *is* these two names; without it they are the stock
        // scripted agent under a name that claims otherwise, and with it they
        // are one agent under two names.
        "evolved" | "advanced_evolved" => (
            vec![genome(true)],
            if champion {
                "advanced_evolved"
            } else {
                "advanced"
            },
        ),
        "advanced" => (Vec::new(), "advanced"),
        "advanced_v1" => (Vec::new(), "advanced_v1"),
        "basic" => (Vec::new(), "basic"),
        "random" => (Vec::new(), "random"),
        // The value net *is* `strategic`: without it the name plays the
        // score-only search under a name that claims otherwise.
        "strategic" => (
            vec![genome(false), value(true)],
            if net { "strategic" } else { "strategic_score" },
        ),
        "strategic_deep" => (vec![genome(false), value(false)], "strategic_deep"),
        _ => (Vec::new(), "basic"),
    };
    AgentProvenance {
        requested: name.to_string(),
        artifacts,
        effective,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A checkout with no trained artifacts is the default state of this
    /// repository — `evolved/` is generated and ignored — so every learned
    /// name must report the scripted agent it really is.
    #[test]
    fn a_bare_checkout_reports_the_agent_that_actually_plays() {
        let dir = "target/test-provenance-bare";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        for (name, effective) in [
            ("evolved", "advanced"),
            ("advanced_evolved", "advanced"),
            ("strategic", "strategic_score"),
        ] {
            let resolved = builtin_provenance(name, dir);
            assert_eq!(resolved.effective, effective, "{name}");
            assert!(resolved.degraded(), "{name}");
            assert!(resolved.untrained(), "{name}");
            assert!(resolved.line().contains("missing"), "{}", resolved.line());
        }
        fs::remove_dir_all(dir).unwrap();
    }

    /// Presence is decided by the loaders the agents use. A file that exists
    /// but cannot load leaves the agent scripted, so provenance must not
    /// call it found.
    #[test]
    fn an_unloadable_artifact_is_not_a_loaded_one() {
        let dir = "target/test-provenance-corrupt";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        fs::write(format!("{dir}/{VALUENET_FILE}"), "{\"sizes\":[1,2]}").unwrap();
        fs::write(format!("{dir}/{CHAMPION_FILE}"), "not json").unwrap();
        let resolved = builtin_provenance("strategic", dir);
        assert_eq!(resolved.effective, "strategic_score");
        assert_eq!(resolved.missing(), vec![CHAMPION_FILE, VALUENET_FILE]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn win_shares_are_a_distribution_over_the_table() {
        let table = [1914.0, 1865.0, 1836.0, 1847.0, 1766.0, 1755.0];
        let shares = win_shares(&table);
        assert!((shares.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!(shares[0] > shares[5]);
        let pair = win_shares(&[1600.0, 1400.0]);
        assert!((pair[0] - expected(1600.0, 1400.0)).abs() < 1e-12);
        let wide = win_shares(&[40_000.0, 0.0]);
        assert!((wide[0] + wide[1] - 1.0).abs() < 1e-9 && wide[0] > 0.999);
    }
}
