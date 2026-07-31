//! Qualified, abstaining action-advantage artifacts.
//!
//! A counterfactual ranker is allowed to replace one scripted `Move` only
//! when a separate reliability head clears its fixed probability threshold.
//! More importantly, the loader treats evaluation evidence as part of the
//! model format: an artifact that did not pass grouped development, blind
//! Standard selection, and untouched Online deployment gates is not a model.
//! Callers can therefore fall back to the scripted expert without accidentally
//! deploying a diagnostic fit that merely happened to serialize.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::action_space::{self, FeatureContext};
use crate::decision_features;
use crate::game::{Action, Game};
use crate::Pos;

pub const FILE: &str = "q_override.json";
pub const SCHEMA: &str = "civvis-q-override-qualified-v3";
pub const RANKER_SCHEMA: &str = "civvis-q-pairwise-v1";
/// Byte identity of the historical JSON regenerated from source commit
/// `90335031354b28eda33eb41dc03fb03fab5f9a92`. This is retained for external
/// provenance checks; runtime behavior is pinned by the decoded-field
/// fingerprint below so harmless JSON whitespace cannot change acceptance.
pub const FROZEN_RANKER_SHA256: &str =
    "2c93f4456b72d1acf548f1994c9ce49569fe158c7b8eb18f4c903b606ce1c463";
pub const FROZEN_RANKER_FINGERPRINT: &str = "fnv1a64:a9b4c4ddc2749250";
pub const OVERRIDE_PROBABILITY: f64 = 0.70;
pub const REQUIRED_REPLICAS: usize = 4;
pub const RANKER_ALTERNATIVES: usize = 3;
pub const RELIABILITY_FOLDS: usize = 5;
pub const RELIABILITY_STEPS: usize = 6_000;
pub const RELIABILITY_RATE: f64 = 0.05;
pub const RELIABILITY_L2: f64 = 0.02;

pub const DEVELOPMENT_SEED: u64 = 1_250_000;
pub const DEVELOPMENT_GAMES: usize = 192;
pub const SELECTION_SEED: u64 = 1_250_192;
pub const SELECTION_GAMES: usize = 96;
pub const DEPLOYMENT_SEED: u64 = 1_251_000;
pub const DEPLOYMENT_GAMES: usize = 96;

/// Fixed thirteen-term reliability representation. It deliberately excludes
/// the 34 empire aggregates and 16 duplicated role flags that overfit the
/// first 52-game experiment. These are rank margin plus safety/progress
/// changes caused by the proposed destination.
pub const RELIABILITY_FEATURES: [&str; 13] = [
    "rank_margin",
    "objective_progress_delta",
    "hostile_threat_delta",
    "hostile_coverage_delta",
    "friendly_strength_delta",
    "adjacent_friends_delta",
    "attack_margin_delta",
    "ranged_support_delta",
    "terrain_defense_delta",
    "exit_fraction_delta",
    "frontier_fraction_delta",
    "hostile_progress_delta",
    "home_progress_delta",
];
pub const RELIABILITY_WIDTH: usize = RELIABILITY_FEATURES.len();

const EPS: f64 = 1e-9;
const DESTINATION_BASE: usize = action_space::KINDS.len() + action_space::LEGACY_NUMERIC_WIDTH;
const RANKER_WIDTH: usize = decision_features::WIDTH + action_space::FEATURE_WIDTH;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReliabilityModel {
    pub means: Vec<f64>,
    pub stddevs: Vec<f64>,
    pub weights: Vec<f64>,
    pub intercept: f64,
    pub constant_probability: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GateEvidence {
    pub profile: String,
    pub seed: u64,
    pub games: usize,
    pub decisions: usize,
    pub passed: bool,
    pub raw_brier: f64,
    pub constant_brier: f64,
    pub reliability_brier: f64,
    pub lift: f64,
    pub lift_se: f64,
    pub override_rate: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Qualification {
    pub status: String,
    pub development: GateEvidence,
    pub selection: GateEvidence,
    pub deployment: GateEvidence,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Artifact {
    pub schema: String,
    pub ranker_schema: String,
    pub ranker_fingerprint: String,
    pub ranker_weights: Vec<f64>,
    pub ranker_feature_width: usize,
    pub reliability_features: Vec<String>,
    pub reliability_feature_width: usize,
    pub replicas: usize,
    pub folds: usize,
    pub steps: usize,
    pub rate: f64,
    pub l2: f64,
    pub override_probability: f64,
    pub reliability: ReliabilityModel,
    pub qualification: Qualification,
}

#[derive(Clone, Debug)]
pub struct QualifiedQOverride {
    artifact: Artifact,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OverrideDecision {
    pub action: Action,
    pub probability: Option<f64>,
    pub rank_margin: Option<f64>,
    pub overridden: bool,
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

/// Stable fingerprint over every behavior-defining ranker field. The trainer
/// copies a frozen ranker into the qualified artifact and the loader checks
/// this digest before accepting it.
pub fn ranker_fingerprint(weights: &[f64], width: usize, replicas: usize) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in RANKER_SCHEMA
        .as_bytes()
        .iter()
        .copied()
        .chain((width as u64).to_le_bytes())
        .chain((replicas as u64).to_le_bytes())
        .chain(
            weights
                .iter()
                .flat_map(|weight| weight.to_bits().to_le_bytes()),
        )
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn validate_gate(
    gate: &GateEvidence,
    profile: &str,
    seed: u64,
    games: usize,
    deployment: bool,
) -> Result<(), String> {
    let metrics = [
        gate.raw_brier,
        gate.constant_brier,
        gate.reliability_brier,
        gate.lift,
        gate.lift_se,
        gate.override_rate,
    ];
    if gate.profile != profile || gate.seed != seed || gate.games != games {
        return Err(format!(
            "{profile} evidence does not match the preregistered seed/count"
        ));
    }
    if !gate.passed || gate.decisions < gate.games || !finite(&metrics) {
        return Err(format!("{profile} evidence is incomplete or did not pass"));
    }
    if gate.decisions > games.saturating_mul(2)
        || !(0.0..=1.0).contains(&gate.raw_brier)
        || !(0.0..=1.0).contains(&gate.constant_brier)
        || !(0.0..=1.0).contains(&gate.reliability_brier)
        || !(-1.0..=1.0).contains(&gate.lift)
        || !(0.0..=1.0).contains(&gate.lift_se)
        || !(0.0..=1.0).contains(&gate.override_rate)
    {
        return Err(format!("{profile} evidence contains impossible metrics"));
    }
    if gate.reliability_brier + EPS >= gate.raw_brier
        || gate.reliability_brier + EPS >= gate.constant_brier
        || gate.lift <= 0.0
        || gate.override_rate + EPS < 0.05
    {
        return Err(format!(
            "{profile} evidence fails the fixed calibration/lift/coverage gate"
        ));
    }
    if deployment && gate.lift - 1.96 * gate.lift_se <= 0.0 {
        return Err("online_deployment evidence lacks a positive 95% lift bound".to_string());
    }
    Ok(())
}

impl QualifiedQOverride {
    pub fn load(path: impl AsRef<Path>) -> Result<QualifiedQOverride, String> {
        let path = path.as_ref();
        let source = fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let artifact: Artifact = serde_json::from_str(&source)
            .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
        Self::from_artifact(artifact).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn load_dir(dir: impl AsRef<Path>) -> Result<QualifiedQOverride, String> {
        Self::load(dir.as_ref().join(FILE))
    }

    pub fn from_artifact(artifact: Artifact) -> Result<QualifiedQOverride, String> {
        if artifact.schema != SCHEMA
            || artifact.ranker_schema != RANKER_SCHEMA
            || artifact.ranker_feature_width != RANKER_WIDTH
            || artifact.ranker_weights.len() != RANKER_WIDTH
            || artifact.replicas != REQUIRED_REPLICAS
            || !finite(&artifact.ranker_weights)
        {
            return Err("ranker schema, width, replicas, or coefficients are invalid".to_string());
        }
        if artifact.ranker_fingerprint != FROZEN_RANKER_FINGERPRINT
            || artifact.ranker_fingerprint
                != ranker_fingerprint(
                    &artifact.ranker_weights,
                    artifact.ranker_feature_width,
                    artifact.replicas,
                )
        {
            return Err(
                "ranker fingerprint does not match the preregistered frozen coefficients"
                    .to_string(),
            );
        }
        let expected_features = RELIABILITY_FEATURES.map(str::to_string).to_vec();
        let model = &artifact.reliability;
        if artifact.reliability_features != expected_features
            || artifact.reliability_feature_width != RELIABILITY_WIDTH
            || model.means.len() != RELIABILITY_WIDTH
            || model.stddevs.len() != RELIABILITY_WIDTH
            || model.weights.len() != RELIABILITY_WIDTH
            || !finite(&model.means)
            || !finite(&model.stddevs)
            || !finite(&model.weights)
            || model.stddevs.iter().any(|value| *value <= 0.0)
            || !model.intercept.is_finite()
            || !model.constant_probability.is_finite()
            || !(0.0..=1.0).contains(&model.constant_probability)
            || artifact.folds != RELIABILITY_FOLDS
            || artifact.steps != RELIABILITY_STEPS
            || (artifact.rate - RELIABILITY_RATE).abs() > EPS
            || (artifact.l2 - RELIABILITY_L2).abs() > EPS
            || (artifact.override_probability - OVERRIDE_PROBABILITY).abs() > EPS
        {
            return Err(
                "reliability schema, normalization, or coefficients are invalid".to_string(),
            );
        }
        if artifact.qualification.status != "qualified" {
            return Err("artifact status is not qualified".to_string());
        }
        validate_gate(
            &artifact.qualification.development,
            "standard_development_oof",
            DEVELOPMENT_SEED,
            DEVELOPMENT_GAMES,
            false,
        )?;
        validate_gate(
            &artifact.qualification.selection,
            "standard_selection",
            SELECTION_SEED,
            SELECTION_GAMES,
            false,
        )?;
        validate_gate(
            &artifact.qualification.deployment,
            "online_deployment",
            DEPLOYMENT_SEED,
            DEPLOYMENT_GAMES,
            true,
        )?;
        Ok(QualifiedQOverride { artifact })
    }

    pub fn artifact(&self) -> &Artifact {
        &self.artifact
    }

    fn score(&self, state: &[f32], action: &[f32]) -> f64 {
        self.artifact
            .ranker_weights
            .iter()
            .zip(state.iter().chain(action))
            .map(|(weight, value)| weight * f64::from(*value))
            .sum()
    }

    fn probability(&self, features: &[f64; RELIABILITY_WIDTH]) -> f64 {
        let model = &self.artifact.reliability;
        let logit = model
            .weights
            .iter()
            .zip(&model.means)
            .zip(&model.stddevs)
            .zip(features)
            .fold(model.intercept, |sum, (((weight, mean), stddev), value)| {
                sum + weight * (value - mean) / stddev
            });
        if logit >= 0.0 {
            1.0 / (1.0 + (-logit).exp())
        } else {
            let exp = logit.exp();
            exp / (1.0 + exp)
        }
    }

    /// Return the expert action unchanged unless this is a same-unit `Move`
    /// decision with a positive rank margin and qualified reliability at or
    /// above 0.70. Unsupported and single-candidate decisions abstain.
    pub fn decide(
        &self,
        game: &Game,
        pid: usize,
        objectives: &[Pos],
        expert: &Action,
    ) -> OverrideDecision {
        let Some(unit) = action_space::acting_unit(expert) else {
            return abstain(expert);
        };
        if action_space::kind_name(expert) != "move" {
            return abstain(expert);
        }
        let legal = game.legal_actions(pid);
        if !legal.contains(expert) {
            return abstain(expert);
        }
        let candidates = action_space::sampled_move_candidates(
            &legal,
            expert,
            RANKER_ALTERNATIVES,
            game.seed ^ u64::from(game.turn) ^ u64::from(unit),
        );
        let siblings = candidates.iter().skip(1);
        let context = FeatureContext::new(game, pid, objectives);
        let state = decision_features::decision_features(game, pid);
        let expert_features = action_space::features_with_context(game, pid, expert, &context);
        let expert_score = self.score(&state, &expert_features);
        let mut best: Option<(Action, Vec<f32>, f64)> = None;
        for sibling in siblings {
            let features = action_space::features_with_context(game, pid, sibling, &context);
            let score = self.score(&state, &features);
            if best
                .as_ref()
                .is_none_or(|(_, _, current)| score > *current + EPS)
            {
                best = Some((sibling.clone(), features, score));
            }
        }
        let Some((sibling, sibling_features, sibling_score)) = best else {
            return abstain(expert);
        };
        let margin = sibling_score - expert_score;
        if margin <= EPS {
            return OverrideDecision {
                action: expert.clone(),
                probability: None,
                rank_margin: Some(margin),
                overridden: false,
            };
        }
        let features = reliability_features(&expert_features, &sibling_features, margin);
        let probability = self.probability(&features);
        let overridden = probability + EPS >= self.artifact.override_probability;
        OverrideDecision {
            action: if overridden { sibling } else { expert.clone() },
            probability: Some(probability),
            rank_margin: Some(margin),
            overridden,
        }
    }
}

fn abstain(expert: &Action) -> OverrideDecision {
    OverrideDecision {
        action: expert.clone(),
        probability: None,
        rank_margin: None,
        overridden: false,
    }
}

pub fn artifact_path(dir: impl AsRef<Path>) -> PathBuf {
    dir.as_ref().join(FILE)
}

/// Construct the fixed low-dimensional row from the expert and ranker's best
/// sibling. Public so the trainer and runtime cannot drift independently.
pub fn reliability_features<T>(expert: &[T], sibling: &[T], margin: f64) -> [f64; RELIABILITY_WIDTH]
where
    T: Copy + Into<f64>,
{
    assert_eq!(expert.len(), action_space::FEATURE_WIDTH);
    assert_eq!(sibling.len(), action_space::FEATURE_WIDTH);
    let delta = |index: usize| {
        sibling[DESTINATION_BASE + index].into() - expert[DESTINATION_BASE + index].into()
    };
    [
        margin,
        delta(10),
        delta(24),
        delta(23),
        delta(20),
        delta(19),
        delta(33),
        delta(34),
        delta(12),
        delta(30),
        delta(29),
        delta(22),
        delta(28),
    ]
}

#[cfg(test)]
pub(crate) fn valid_test_artifact() -> Artifact {
    #[derive(Deserialize)]
    struct Fixture {
        weights: Vec<f64>,
    }
    let ranker_weights = serde_json::from_str::<Fixture>(include_str!(
        "../tests/fixtures/q-pairwise-base.json"
    ))
    .expect("frozen ranker fixture parses")
    .weights;
    let gate = |profile: &str, seed, games| GateEvidence {
        profile: profile.to_string(),
        seed,
        games,
        decisions: games * 2,
        passed: true,
        raw_brier: 0.25,
        constant_brier: 0.24,
        reliability_brier: 0.20,
        lift: 0.02,
        lift_se: if profile == "online_deployment" {
            0.005
        } else {
            0.01
        },
        override_rate: 0.10,
    };
    Artifact {
        schema: SCHEMA.to_string(),
        ranker_schema: RANKER_SCHEMA.to_string(),
        ranker_fingerprint: ranker_fingerprint(&ranker_weights, RANKER_WIDTH, REQUIRED_REPLICAS),
        ranker_weights,
        ranker_feature_width: RANKER_WIDTH,
        reliability_features: RELIABILITY_FEATURES.map(str::to_string).to_vec(),
        reliability_feature_width: RELIABILITY_WIDTH,
        replicas: REQUIRED_REPLICAS,
        folds: RELIABILITY_FOLDS,
        steps: RELIABILITY_STEPS,
        rate: RELIABILITY_RATE,
        l2: RELIABILITY_L2,
        override_probability: OVERRIDE_PROBABILITY,
        reliability: ReliabilityModel {
            means: vec![0.0; RELIABILITY_WIDTH],
            stddevs: vec![1.0; RELIABILITY_WIDTH],
            weights: vec![0.0; RELIABILITY_WIDTH],
            intercept: 2.0,
            constant_probability: 0.5,
        },
        qualification: Qualification {
            status: "qualified".to_string(),
            development: gate(
                "standard_development_oof",
                DEVELOPMENT_SEED,
                DEVELOPMENT_GAMES,
            ),
            selection: gate("standard_selection", SELECTION_SEED, SELECTION_GAMES),
            deployment: gate("online_deployment", DEPLOYMENT_SEED, DEPLOYMENT_GAMES),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{valid_test_artifact, QualifiedQOverride};
    use crate::ai::{AdvancedAi, Ai};
    use crate::game::{Action, Game};
    use std::fs;

    fn move_pair(game: &Game) -> (Action, Action) {
        let moves: Vec<Action> = game
            .legal_actions(0)
            .into_iter()
            .filter(|action| matches!(action, Action::Move { .. }))
            .collect();
        moves
            .windows(2)
            .find(|pair| {
                crate::action_space::acting_unit(&pair[0])
                    == crate::action_space::acting_unit(&pair[1])
            })
            .map(|pair| (pair[0].clone(), pair[1].clone()))
            .expect("one starting unit has two moves")
    }

    #[test]
    fn only_fully_qualified_evidence_loads() {
        let valid = valid_test_artifact();
        assert!(QualifiedQOverride::from_artifact(valid.clone()).is_ok());

        let mut substituted = valid.clone();
        substituted.ranker_weights[0] = 1.0;
        substituted.ranker_fingerprint = super::ranker_fingerprint(
            &substituted.ranker_weights,
            substituted.ranker_feature_width,
            substituted.replicas,
        );
        assert!(QualifiedQOverride::from_artifact(substituted).is_err());

        let mut failed = valid.clone();
        failed.qualification.selection.passed = false;
        assert!(QualifiedQOverride::from_artifact(failed).is_err());

        let mut retuned = valid.clone();
        retuned.l2 = 0.01;
        assert!(QualifiedQOverride::from_artifact(retuned).is_err());

        let mut impossible = valid.clone();
        impossible.qualification.deployment.lift_se = -0.01;
        assert!(QualifiedQOverride::from_artifact(impossible).is_err());

        let mut weak_external = valid.clone();
        weak_external.qualification.deployment.lift_se = 0.02;
        assert!(QualifiedQOverride::from_artifact(weak_external).is_err());

        let mut stale = valid;
        stale.reliability_features.swap(0, 1);
        assert!(QualifiedQOverride::from_artifact(stale).is_err());
    }

    #[test]
    fn qualified_head_overrides_and_low_confidence_head_abstains() {
        let game = Game::new(3, 24, 16, 78_201, 40, 0);
        let (expert, sibling) = move_pair(&game);
        let objective = match sibling {
            Action::Move { to, .. } => to,
            _ => unreachable!(),
        };
        let model = QualifiedQOverride::from_artifact(valid_test_artifact()).unwrap();
        let decision = model.decide(&game, 0, &[objective], &expert);
        assert!(
            decision.overridden,
            "high-confidence fixture should override"
        );
        assert_ne!(decision.action, expert);

        let mut artifact = valid_test_artifact();
        artifact.reliability.intercept = 0.0;
        let abstainer = QualifiedQOverride::from_artifact(artifact).unwrap();
        let decision = abstainer.decide(&game, 0, &[objective], &expert);
        assert!(!decision.overridden);
        assert_eq!(decision.action, expert);
    }

    #[test]
    fn missing_or_unqualified_files_execute_the_expert_exactly() {
        let dir = "target/test-q-override-fail-closed";
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();
        fs::write(super::artifact_path(dir), b"{}").unwrap();

        let mut expert_game = Game::new(3, 24, 16, 78_202, 40, 0);
        let mut fallback_game = expert_game.clone();
        let mut expert = AdvancedAi::new();
        let mut fallback = AdvancedAi::qualified_q_override_or_expert(dir);
        expert.take_turn(&mut expert_game, 0);
        fallback.take_turn(&mut fallback_game, 0);
        assert_eq!(
            serde_json::to_vec(&expert_game).unwrap(),
            serde_json::to_vec(&fallback_game).unwrap()
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
