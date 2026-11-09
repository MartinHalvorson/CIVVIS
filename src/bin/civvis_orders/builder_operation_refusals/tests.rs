use super::*;
use crate::{HostOrderRefusals, IssuedOrder, OrderCheck, Verdict, ORDER_REFUSAL_STRIKES};

fn fixture() -> (Game, BTreeMap<u32, i64>, u32) {
    let (snapshot, mut state) = crate::tests::local_barbarian_defense_board();
    let unit = state.units.iter_mut().find(|unit| unit.id == 100).unwrap();
    unit.kind = "UNIT_BUILDER".into();
    unit.build_charges = Some(3);
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
    let uid = *mirror
        .civ6_of
        .iter()
        .find(|(_, host)| **host == 100)
        .unwrap()
        .0;
    assert_eq!(mirror.game.units[&uid].kind, "builder");
    (mirror.game, mirror.civ6_of, uid)
}

fn check(verb: &str, verdict: Verdict) -> OrderCheck {
    OrderCheck {
        order: IssuedOrder {
            kind: "unit".into(),
            subject: Some(100),
            verb: Some(verb.into()),
            pos: None,
        },
        verdict,
    }
}

fn failures(verb: &str) -> HostOrderRefusals {
    let mut refusals = HostOrderRefusals::default();
    for _ in 0..ORDER_REFUSAL_STRIKES {
        refusals.observe(&[check(verb, Verdict::Failed("no_improvement".into()))], 92);
    }
    refusals
}

#[test]
fn withheld_harvest_is_keyed_to_the_builder_without_mutating_the_mirror() {
    let (observed, mapped, uid) = fixture();
    let operations = refused_operations(&observed, &mapped, 93, &failures("HARVEST_RESOURCE"));
    assert_eq!(operations.len(), 1);
    assert_eq!(
        operations[&uid],
        BTreeSet::from([Name::new("harvest_resource")])
    );
    assert!(observed.blocked_improvements.is_empty());
    assert_eq!(observed.units[&uid].charges, 3);
}

#[test]
fn clearing_and_planting_use_the_exact_exported_verbs() {
    for (verb, expected) in [
        (
            "REMOVE_FEATURE",
            vec!["chop_woods", "chop_rainforest", "clear_marsh"],
        ),
        ("PLANT_FOREST", vec!["plant_woods"]),
    ] {
        let (game, mapped, uid) = fixture();
        let operations = refused_operations(&game, &mapped, 93, &failures(verb));
        assert_eq!(operations.len(), 1);
        assert_eq!(
            operations[&uid],
            expected.into_iter().map(Name::new).collect()
        );
    }
}

#[test]
fn absent_unconfirmed_expired_or_verified_failures_leave_work_available() {
    let (game, mapped, _) = fixture();
    let mut one_failure = HostOrderRefusals::default();
    one_failure.observe(
        &[check(
            "HARVEST_RESOURCE",
            Verdict::Failed("no_improvement".into()),
        )],
        92,
    );
    let mut unknown = HostOrderRefusals::default();
    unknown.observe(&[check("HARVEST_RESOURCE", Verdict::Unverifiable)], 92);
    let mut cleared = failures("HARVEST_RESOURCE");
    cleared.observe(&[check("HARVEST_RESOURCE", Verdict::Verified)], 93);
    for (turn, refusals) in [
        (93, HostOrderRefusals::default()),
        (93, one_failure),
        (93, unknown),
        (102, failures("HARVEST_RESOURCE")),
        (93, cleared),
        (93, failures("MOVE_TO")),
    ] {
        assert!(refused_operations(&game, &mapped, turn, &refusals).is_empty());
        assert!(game.blocked_improvements.is_empty());
    }
}

#[test]
fn unmapped_foreign_or_non_builder_units_cannot_block_the_ground() {
    for mode in 0..3 {
        let (mut game, mut mapped, uid) = fixture();
        match mode {
            0 => {
                mapped.clear();
            }
            1 => {
                game.units.get_mut(&uid).unwrap().owner = 1;
            }
            _ => {
                game.units.get_mut(&uid).unwrap().kind = Name::new("warrior");
            }
        }
        assert!(refused_operations(&game, &mapped, 93, &failures("HARVEST_RESOURCE")).is_empty());
        assert!(game.blocked_improvements.is_empty());
    }
}
