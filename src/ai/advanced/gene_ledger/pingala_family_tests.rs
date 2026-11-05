use super::*;

#[test]
fn forced_pingala_launch_version_reports_the_same_exclusive_flags_it_runs() {
    let old = "pingala-follows-research";
    let new = "pingala-follows-research-2";
    let reported = deployment_treatments_with_forced_live(&[new]);
    assert!(reported.contains(&new));
    assert!(
        !reported.contains(&old),
        "native metadata must not report the sibling cleared by the enable function"
    );
    let mut ai = AdvancedAi::new();
    ai.enable_pingala_follows_research();
    ai.apply_gene_ledger_with_forced_live(&[new]);
    assert!(ai.pingala_follows_research_2);
    assert!(!ai.pingala_follows_research);
}

#[test]
fn the_pingala_metadata_family_retains_unrelated_treatments_in_either_order() {
    let old = "pingala-follows-research";
    let new = "pingala-follows-research-2";
    let unrelated = "reyna-follows-revenue";
    assert_eq!(
        collapse_mutually_exclusive_treatments(vec![old], &[]),
        vec![old]
    );
    for tags in [vec![unrelated, old, new], vec![new, old, unrelated]] {
        let reported = collapse_mutually_exclusive_treatments(tags, &[new]);
        assert_eq!(reported.len(), 2);
        assert!(reported.contains(&unrelated));
        assert!(reported.contains(&new));
        assert!(!reported.contains(&old));
    }
}
