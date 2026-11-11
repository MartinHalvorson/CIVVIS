use super::*;

#[test]
fn district_rows_match_the_ordered_map_through_mutations() {
    // Intern in reverse lexical order: IDs must never determine iteration.
    let names: Vec<Name> = (0..24)
        .rev()
        .map(|i| Name::new(&format!("district_rows_same_prefix_{i:02}")))
        .collect();
    let mut districts = Districts::default();
    let mut reference = BTreeMap::<Name, Vec<Pos>>::new();
    let mut random = 917_u64;
    for turn in 0..2_000 {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let name = names[(random >> 32) as usize % names.len()];
        if random & 7 == 0 {
            assert_eq!(districts.remove(name), reference.remove(&name));
        } else {
            let pos = (turn % 41, turn / 41);
            districts.insert(name, pos);
            reference.entry(name).or_default().push(pos);
        }
        let expected: Vec<_> = reference
            .iter()
            .flat_map(|(name, positions)| positions.iter().map(move |pos| (name, pos)))
            .collect();
        assert_eq!(districts.iter().collect::<Vec<_>>(), expected);
        assert_eq!(
            districts.keys().collect::<Vec<_>>(),
            expected.iter().map(|p| p.0).collect::<Vec<_>>()
        );
        assert_eq!(districts.len(), expected.len());
        assert_eq!(districts.is_empty(), expected.is_empty());
        for name in &names {
            let positions = reference.get(name).map(Vec::as_slice).unwrap_or(&[]);
            assert_eq!(districts.positions(*name), positions);
            assert_eq!(districts.get(*name), positions.first());
            assert_eq!(districts.contains_key(*name), !positions.is_empty());
        }
    }
    let saved = serde_json::to_string(&districts).unwrap();
    let mut cloned = districts.clone();
    cloned.insert(names[0], (-99, -99));
    cloned.clear();
    assert!(cloned.is_empty());
    assert_eq!(serde_json::to_string(&districts).unwrap(), saved);
}

#[test]
fn district_save_shapes_and_lexical_order_are_unchanged() {
    let encoded = r#"{"neighborhood":[[9,4],[3,2]],"empty":[],"campus":[1,8]}"#;
    let mut districts: Districts = serde_json::from_str(encoded).unwrap();
    assert_eq!(districts.len(), 3);
    assert!(!districts.contains_key(Name::new("empty")));
    assert_eq!(districts.get(Name::new("neighborhood")), Some(&(9, 4)));
    assert_eq!(
        districts.positions(Name::new("neighborhood")),
        &[(9, 4), (3, 2)]
    );
    assert_eq!(districts.positions(Name::new("absent")), &[]);
    assert_eq!(
        serde_json::to_string(&districts).unwrap(),
        r#"{"campus":[1,8],"empty":[],"neighborhood":[[9,4],[3,2]]}"#
    );
    assert_eq!(districts.remove(Name::new("empty")), Some(vec![]));
    assert_eq!(districts.remove(Name::new("empty")), None);
    let pairs: Vec<_> = (&districts)
        .into_iter()
        .map(|(n, p)| (n.as_str(), *p))
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("campus", (1, 8)),
            ("neighborhood", (9, 4)),
            ("neighborhood", (3, 2))
        ]
    );
}

#[test]
fn district_save_duplicate_names_keep_the_last_value() {
    let districts: Districts =
        serde_json::from_str(r#"{"zeta":[1,2],"alpha":[3,4],"zeta":[[5,6],[7,8]]}"#).unwrap();
    assert_eq!(districts.positions(Name::new("zeta")), &[(5, 6), (7, 8)]);
    assert_eq!(
        serde_json::to_string(&districts).unwrap(),
        r#"{"alpha":[3,4],"zeta":[[5,6],[7,8]]}"#
    );
}

#[test]
fn destroying_a_placement_keeps_other_instances_and_removes_empty_kinds() {
    let name = Name::new("neighborhood");
    let mut districts = Districts::default();
    districts.insert(name, (1, 2));
    districts.insert(name, (3, 4));
    districts.insert(name, (1, 2));
    districts.insert(Name::new("campus"), (5, 6));
    districts.remove_position(name, (1, 2));
    assert_eq!(districts.positions(name), &[(3, 4)]);
    districts.remove_position(name, (3, 4));
    districts.remove_position(name, (3, 4));
    assert!(!districts.contains_key(name));
    assert_eq!(districts.get(Name::new("campus")), Some(&(5, 6)));
    assert_eq!(
        serde_json::to_string(&districts).unwrap(),
        r#"{"campus":[5,6]}"#
    );
}
