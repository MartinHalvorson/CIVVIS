use super::*;
use civvis::game::Item;
use civvis::rules::Rules;
use std::collections::BTreeSet;

fn shipped() -> BTreeSet<String> {
    let raw = include_str!("../../../data/civ6_type_names.json");
    serde_json::from_str::<Vec<String>>(raw)
        .expect("the type-name snapshot parses")
        .into_iter()
        .collect()
}

/// CIVVIS concepts Civilization VI has no build order for at all.
///
/// A National Park is not an Improvement row in the shipped ruleset, an
/// Antiquity Site and a Shipwreck are Resources an Archaeologist consumes,
/// and a Rock Concert is a unit ability. Naming them here says "checked, and
/// there is deliberately nothing to map" rather than leaving them to the
/// uppercase fallthrough, which would emit a name and have it dropped.
/// `PROJECT_REPAIR_ENCAMPMENT` is the one entry here that is a *deferral*
/// rather than an absence: Civilization VI repairs a pillaged Encampment by
/// ordering the district again, which needs a plot the `Item::Project` arm
/// cannot see. `civ6_build_name` records why it is left untranslated and
/// where the repair belongs. Listing it keeps that decision declared instead
/// of indistinguishable from a name nobody has checked.
/// Promotions that belong to a unit Civilization VI does not have.
///
/// The Nihang is CIVVIS content: `data/promotions.json` gives it its own
/// promotion class and the shipped ruleset has no row for any of them. They
/// can never reach the wire on a live seat — the mirror cannot build a unit
/// the host never fielded — so this is an absence, not an unfixed mapping.
/// Naming them keeps that judgement declared and re-checked on every run.
const CIVVIS_ONLY_PROMOTIONS: [&str; 7] = [
    "PROMOTION_CHAKRAM",
    "PROMOTION_DUMALLA",
    "PROMOTION_JANGI_KARA",
    "PROMOTION_JANGI_MOJEH",
    "PROMOTION_SANJO",
    "PROMOTION_TEGH",
    "PROMOTION_TREHSOOL_MUKH",
];

const NO_CIV6_EQUIVALENT: [&str; 5] = [
    "IMPROVEMENT_NATIONAL_PARK",
    "IMPROVEMENT_ARCHAEOLOGICAL_DIG",
    "IMPROVEMENT_SHIPWRECK_EXCAVATION",
    "IMPROVEMENT_ROCK_CONCERT",
    "PROJECT_REPAIR_ENCAMPMENT",
];

/// The exact regression, pinned: repairing a pillaged Government Plaza
/// must not re-introduce CIVVIS's own spelling.
///
/// Live run `civvis-20260803T094641Z` emitted `DISTRICT_GOVERNMENT_PLAZA`
/// 26 times while `civ6_district_type` had already mapped it to
/// `DISTRICT_GOVERNMENT` — because the repair arm formatted its own name.
#[test]
fn repairing_a_district_uses_the_same_spelling_as_building_one() {
    use civvis::game::{Game, Item};
    let mut board = Game::new(2, 8, 8, 7, 100, 0);
    let pos = (3, 3);
    board.map.tiles.get_mut(&pos).expect("a tile").district =
        Some(civvis::name::Name::new("government_plaza"));

    let repaired = civ6_live_build_name(
        &Item::Repair {
            repair: civvis::name::Name::new("district"),
            pos,
        },
        &board,
    )
    .expect("a district repair names a district");

    assert_eq!(
        repaired, "DISTRICT_GOVERNMENT",
        "the repair path must use civ6_district_type, not its own uppercase"
    );
    assert_eq!(
        repaired,
        civ6_district_type(&civvis::name::Name::new("government_plaza")),
        "one table, one formatter"
    );
    assert!(
        shipped().contains(&repaired),
        "and the result must be a name Civilization VI actually ships"
    );
}

#[test]
fn the_snapshot_is_the_shipped_ruleset_and_not_our_own_mod() {
    let shipped = shipped();
    assert!(
        shipped.len() > 400,
        "the snapshot has {} names, too few to be Civilization VI's ruleset",
        shipped.len()
    );
    assert!(
        shipped.contains("BUILDING_MUSEUM_ARTIFACT"),
        "the real name of the building 248 live orders never built"
    );
    assert!(
        !shipped.contains("BUILDING_ARCHAEOLOGICAL_MUSEUM"),
        "CIVVIS's own spelling is in the snapshot, so the harvest read our \
             control mod back as evidence about the game — rerun \
             tools/civ6_type_names.py, which excludes DLC/CivvisControl"
    );
}

#[test]
fn every_name_the_order_channel_can_emit_exists_in_civilization_vi() {
    let rules = Rules::shipped();
    let shipped = shipped();
    let mut missing: Vec<(&str, String, String)> = Vec::new();

    let mut check = |kind: &'static str, name: &str, emitted: String| {
        if !shipped.contains(&emitted)
            && !NO_CIV6_EQUIVALENT.contains(&emitted.as_str())
            && !CIVVIS_ONLY_PROMOTIONS.contains(&emitted.as_str())
        {
            missing.push((kind, name.to_string(), emitted));
        }
    };

    // ⚠ Drive `civ6_build_name`, the function the order channel actually
    // calls, rather than the per-kind helpers. Dispatch is where this has
    // gone wrong before: #959 put three divergent wonder spellings into
    // `civ6_building_type` while the `Item::Wonder` arm still formatted its
    // own name one line away, so the repair never reached the path that
    // emitted them. A test that reproduces the mapping instead of invoking
    // it reproduces that mistake too — this one wrote out the same stale
    // uppercase and reported three false failures against correct code.
    let anywhere: civvis::Pos = (0, 0);
    for name in rules.units.keys() {
        let item = Item::Unit { unit: *name };
        check(
            "unit",
            name.as_str(),
            civ6_build_name(&item).expect("a unit name"),
        );
    }
    for name in rules.districts.keys() {
        let item = Item::District {
            district: *name,
            pos: anywhere,
        };
        check(
            "district",
            name.as_str(),
            civ6_build_name(&item).expect("a district name"),
        );
    }
    for name in rules.buildings.keys() {
        let item = Item::Building { building: *name };
        check(
            "building",
            name.as_str(),
            civ6_build_name(&item).expect("a building name"),
        );
    }
    for name in rules.wonders.keys() {
        let item = Item::Wonder {
            wonder: *name,
            pos: anywhere,
        };
        check(
            "wonder",
            name.as_str(),
            civ6_build_name(&item).expect("a wonder name"),
        );
    }
    for name in rules.projects.keys() {
        let item = Item::Project { project: *name };
        check(
            "project",
            name.as_str(),
            civ6_build_name(&item).expect("a project name"),
        );
    }
    // Improvements are ordered by builders, not produced in a city, so they
    // reach the wire through their own helper rather than `civ6_build_name`.
    for name in rules.improvements.keys() {
        check("improvement", name.as_str(), civ6_improvement_type(name));
    }
    // ⚠ AND THE REPAIR PATH, which is a DIFFERENT FUNCTION.
    //
    // The first version of this audit drove `civ6_build_name` only. That
    // missed `civ6_live_build_name`'s `Item::Repair` arm, which formatted
    // its own uppercase — and live run `civvis-20260803T094641Z` then
    // emitted `DISTRICT_GOVERNMENT_PLAZA` 26 times, all discarded, on a
    // revision where every other path spelled it `DISTRICT_GOVERNMENT`.
    //
    // A name audit is only worth what it covers. Drive every function that
    // can put a name on the wire, not the one that is easiest to reach.
    let board = civvis::game::Game::new(2, 8, 8, 1, 100, 0);
    for name in rules.districts.keys() {
        let repaired = civ6_district_type(name);
        check("district-repair", name.as_str(), repaired);
    }
    for name in rules.buildings.keys() {
        let item = Item::Repair {
            repair: *name,
            pos: (0, 0),
        };
        if let Some(emitted) = civ6_live_build_name(&item, &board) {
            check("building-repair", name.as_str(), emitted);
        }
    }
    // ★★★ AND PROMOTIONS, WHICH THIS AUDIT DID NOT COVER UNTIL A WHOLE
    // FAMILY OF THEM WAS WRONG.
    //
    // The comment above says a name audit is only worth what it covers, and
    // then the audit covered five families out of six. Every spy promotion
    // went out as `PROMOTION_<NAME>` when the host ships
    // `PROMOTION_SPY_<NAME>` — 259-341 refusals a game, the seat's largest
    // refusal category, and a bridge-health drop from 96% to 87% that no
    // test noticed. Both sources are driven here: the ruleset's own table
    // and `Game::SPY_PROMOTIONS`, which is not in it.
    for name in rules.promotions.keys() {
        check(
            "promotion",
            name.as_str(),
            civ6_unit_promotion_name(name.as_str()),
        );
    }
    for name in civvis::game::Game::SPY_PROMOTIONS {
        check("spy-promotion", name, civ6_unit_promotion_name(name));
    }

    assert!(
        missing.is_empty(),
        "these names would be written to the order channel and silently \
             discarded by the host — add an arm to the matching civ6_*_type \
             function, or to NO_CIV6_EQUIVALENT if the game genuinely has no \
             build order for it:\n{}",
        missing
            .iter()
            .map(|(kind, name, emitted)| format!("  {kind:11} {name:32} -> {emitted}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
