use super::*;

#[test]
fn one_air_refusal_does_not_blacklist_a_valid_later_same_turn_retry() {
    let dir = std::env::temp_dir().join(format!("civvis-air-retry-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("events.jsonl");
    // Actual 060034 native sequence: the same Bomber, plot and turn are
    // refused, then accepted; defender HP100→99 and wall HP400→367.
    std::fs::write(
        &path,
        concat!(
            "{\"kind\":\"range_attack_refused\",\"turn\":196,\"unit\":9306136,\"verb\":\"AIR_ATTACK\",\"x\":33,\"y\":16}\n",
            "{\"kind\":\"strike\",\"turn\":196,\"unit\":9306136,\"verb\":\"AIR_ATTACK\",\"x\":33,\"y\":16}\n",
            "{\"kind\":\"war_refused\",\"turn\":196,\"unit\":100,\"verb\":\"AIR_ATTACK\",\"x\":26,\"y\":20}\n",
            "{\"kind\":\"range_attack_refused\",\"turn\":196,\"unit\":101,\"verb\":\"RANGE_ATTACK\",\"x\":33,\"y\":16}\n",
            "{\"kind\":\"war_refused\",\"turn\":196,\"unit\":102,\"verb\":\"ATTACK\",\"x\":33,\"y\":16}\n",
            "{\"kind\":\"range_attack_refused\",\"turn\":196,\"unit\":103,\"x\":33,\"y\":16}\n",
            "{\"kind\":\"range_attack_refused\",\"turn\":195,\"unit\":104,\"verb\":\"RANGE_ATTACK\",\"x\":33,\"y\":16}\n",
        ),
    )
    .unwrap();
    let refused = refused_strikes_on(&path, 196);
    assert!(!refused.contains(&(9306136, 33, 16)));
    assert!(!refused.contains(&(100, 26, 20)));
    assert_eq!(
        refused,
        [(101, 33, 16), (102, 33, 16), (103, 33, 16)].into()
    );
    assert!(refused_strikes_on(&path, 197).is_empty());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
