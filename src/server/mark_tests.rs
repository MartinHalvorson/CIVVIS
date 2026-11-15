use super::*;
use serde_json::json;

fn tile() -> Value {
    json!({
        "pos": [12, -7],
        "terrain": "grassland",
        "yields": {"food": 2.0, "production": 1.0, "gold": 0.0},
        "appeal": 1,
        "feature": null,
        "hills": false,
        "resource": "wheat",
        "improvement": "farm",
        "owner": 3,
        "owner_city_name": "Ur",
        "river_edges": [0, 2],
    })
}

/// Equal tiles mark equal, and every single-field change a frame can make
/// marks differently: a value, a kind (`null` against `false` against `0`),
/// a number's reading (`1` against `1.0`), an array element, a key.
#[test]
fn a_mark_moves_with_any_one_change_to_a_tile() {
    let base = tile();
    assert_eq!(tile_mark(&base), tile_mark(&tile()));
    let mut variants = Vec::new();
    for (key, value) in [
        ("terrain", json!("plains")),
        ("appeal", json!(2)),
        ("appeal", json!(1.0)),
        ("feature", json!(false)),
        ("feature", json!(0)),
        ("hills", json!(true)),
        ("resource", json!("wheat ")),
        ("improvement", Value::Null),
        ("owner", json!(4)),
        ("owner_city_name", json!("Uruk")),
        ("river_edges", json!([0, 3])),
        ("river_edges", json!([2, 0])),
        ("pos", json!([12, -6])),
    ] {
        let mut changed = tile();
        changed[key] = value;
        variants.push(changed);
    }
    let mut extra = tile();
    extra["tourism"] = json!(0.0);
    variants.push(extra);
    let mut yields = tile();
    yields["yields"]["gold"] = json!(0.5);
    variants.push(yields);
    let mut marks: Vec<u64> = variants.iter().map(tile_mark).collect();
    marks.push(tile_mark(&base));
    let distinct: std::collections::BTreeSet<u64> = marks.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        marks.len(),
        "every variant marks apart from the rest"
    );
}
