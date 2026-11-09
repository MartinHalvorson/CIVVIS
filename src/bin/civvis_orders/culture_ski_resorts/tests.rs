use super::*;

#[test]
fn translates_adjacent_target_and_staging_in_host_coordinates() {
    let mut orders = vec![Order {
        kind: "research",
        subject: None,
        verb: Some("TECH_COMPUTERS".into()),
        pos: None,
    }];
    let site = (9, 11);
    let step = (6, 10);
    assert_eq!(
        append_jobs(
            &BTreeMap::from([(1, 9371652), (2, 6553613)]),
            vec![(1, site, None), (2, site, Some(step)), (3, site, None)],
            &mut orders
        ),
        2
    );
    assert_eq!(orders[0].verb.as_deref(), Some("TECH_COMPUTERS"));
    assert_eq!(orders[1].subject, Some(9371652));
    assert_eq!(
        orders[1].verb.as_deref(),
        Some("IMPROVE:IMPROVEMENT_SKI_RESORT")
    );
    assert_eq!(
        orders[1].pos,
        Some(civvis::hex::axial_to_offset(site.0, site.1))
    );
    assert_eq!(orders[2].verb.as_deref(), Some("MOVE_TO"));
    assert_eq!(
        orders[2].pos,
        Some(civvis::hex::axial_to_offset(step.0, step.1))
    );
}
