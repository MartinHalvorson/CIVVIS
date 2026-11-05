use super::Weights;

#[test]
fn gene_names_match_the_vector() {
    let w = Weights::default();
    assert_eq!(w.to_vec().len(), Weights::gene_names().len());
    assert_eq!(w.to_vec().len(), Weights::bounds().len());
}

/// The `advanced_holy_priority` treatment's district weights, pinned so the
/// arm keeps measuring the axis it is named for.
///
/// ⚠ **This is no longer the shipped agent.** `d_holy` 5.6 measured +20 Elo
/// over 1200 paired maps at `ai_eval`'s 4p 24x16 defaults (#1469), shipped
/// onto `AdvancedAi::new()`, and was **reverted** the same day: at the
/// deployment shape with all six victories it is parity (+2, CI -46..+50),
/// and on the promotion matrix it is -44 with sign p=0.0016 against. The
/// weights survive as the treatment arm; see `docs/EVAL.md` 2026-08-10.
///
/// The pin still earns its place, because the arm is only worth running if
/// it carries the configuration those numbers describe. The
/// roster-composite screen showed how easily that is lost: `d_commercial`
/// at 5.55 cost **19 of the 20 points** and dropped religious wins 470 to
/// 392 (#1486).
///
/// ⚠ **Note what that rules out.** 5.55 is still *below* 5.6, so the harm
/// arrived with the ordering intact — an ordering assertion passes straight
/// through this regression and is worthless here. The protective property is
/// the **margin**, and two data points (margin 2.6 pays, margin 0.05 does
/// not) do not locate a safe threshold. So this pins the measured values
/// themselves rather than inventing a bound the evidence cannot support.
///
/// Scope is `Weights::advanced()` only — league and evolved entrants carry
/// their own vectors and are entitled to any values they like.
#[test]
fn the_holy_priority_arm_keeps_the_weights_its_numbers_describe() {
    let w = Weights::advanced();
    assert_eq!(
        [w.d_holy, w.d_campus, w.d_commercial, w.d_theater],
        [5.6, 4.0, 3.0, 1.0],
        "the advanced_holy_priority weights moved. Every number recorded \
             for that arm was taken at these four values, and #1486 measured 19 \
             Elo lost from a change to d_commercial alone that never even \
             reversed the ordering. Re-measure before repinning."
    );
}

#[test]
fn every_gene_default_sits_inside_its_own_bounds() {
    let v = Weights::default().to_vec();
    for (index, (lo, hi)) in Weights::bounds().iter().enumerate() {
        assert!(
            v[index] >= *lo && v[index] <= *hi,
            "{} default {} outside [{lo}, {hi}]",
            Weights::gene_names()[index],
            v[index]
        );
    }
}

#[test]
fn gene_vector_children_preserve_non_gene_policy_state() {
    let legacy_artifact: Weights = serde_json::from_str("{}").expect("empty legacy weights");
    assert_eq!(legacy_artifact.policy_deck, super::PolicyDeck::Live);
    assert_eq!(Weights::default().policy_deck, super::PolicyDeck::Legacy);
    let template = Weights {
        pol_food: 0.11,
        pol_production: 0.22,
        pol_gold: 0.33,
        pol_science: 0.44,
        pol_culture: 0.55,
        pol_faith: 0.66,
        pol_military: 0.77,
        pol_swap_margin: 0.88,
        policy_deck: super::PolicyDeck::Live,
        dedication_choice: super::DedicationChoice::Alphabetical,
        ..Weights::default()
    };
    assert_eq!(
        Weights::from_vec_like(&template.to_vec(), &template),
        template,
        "a templated gene reconstruction must not change non-gene policy state"
    );
}
