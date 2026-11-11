//! `generals-lead-the-army` on the live bridge: keep each Great General and
//! Comandante General with a front of our land army instead of retiring it.
//! The gene's reasoning lives in `src/ai/advanced/generals_lead_the_army.rs`.
//!
//! A post is one of our land soldiers. Its worth is the number of our land
//! soldiers within the aura (two tiles, the soldier itself included) plus
//! twice the visible enemy soldiers within four tiles (capped at four), so a
//! fighting front outranks a quiet garrison of the same size. Posts are taken
//! greedily, each at least `FRONT_SEPARATION` tiles from every post already
//! taken, and each post takes the nearest General still free. A General that
//! already stands within one tile of its post holds. Generals beyond the posts
//! are left to the bridge's ordinary activation, except Comandantes: the
//! first [`COMANDANTES_KEPT`] of them wait with our largest group of soldiers,
//! however small, because their aura never expires and a scattered army
//! gathers again (see [`plan`]).

use std::collections::BTreeMap;

use civvis::mirror::{StateSnapshot, StateUnit};

use super::offset_distance;

/// The aura's reach: land units within two tiles of the General.
pub(super) const AURA_RADIUS: i32 = 2;
/// How far an enemy soldier may stand from a post and still mark it a front.
const THREAT_RADIUS: i32 = 4;
/// A second General leads a second front only this far from the first post.
const FRONT_SEPARATION: i32 = 6;
/// The fewest own land soldiers inside the aura that make a post worth a
/// General, unless the post faces an enemy.
const MIN_LED: usize = 2;

/// Comandante Generals kept on the board when no post is worth one: enough
/// for two fronts. Live Deity Gran Colombia `civvis-20261111T172329Z` led its
/// first Comandante from turn 31 and retired it on turn 44, the first frame
/// no two soldiers stood within two tiles of each other, and retired the
/// next five the same way (turns 53, 89, 91, 131 and 134); it never declared
/// a war with one on the board.
const COMANDANTES_KEPT: usize = 2;

/// Whether this General's aura lasts the whole game (`CLASS_ALL_ERAS`). A
/// Great General's covers two eras, so an idle one is still better retired.
fn is_comandante(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("UNIT_COMANDANTE_GENERAL")
}

/// Whether this unit kind is a General whose aura the gene keeps.
pub(super) fn is_general(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("UNIT_GREAT_GENERAL")
        || kind.eq_ignore_ascii_case("UNIT_COMANDANTE_GENERAL")
}

/// A land combat unit the aura strengthens: a promotion class that is neither
/// naval nor air, with a melee or ranged strength, and not a Great Person.
fn is_land_soldier(unit: &StateUnit) -> bool {
    if unit.great_person.is_some() || (unit.combat <= 0.0 && unit.ranged <= 0.0) {
        return false;
    }
    match unit.class.as_deref() {
        Some(class) => {
            let class = class.to_ascii_uppercase();
            class.starts_with("PROMOTION_CLASS_")
                && !class.contains("NAVAL")
                && !class.contains("AIR")
                && !class.contains("SUPPORT")
        }
        None => false,
    }
}

/// Visible enemy soldiers: majors and city-states we are at war with, and
/// every hostile (barbarians, Free Cities).
fn enemy_positions(state: &StateSnapshot) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = Vec::new();
    for rival in state.rivals.iter().filter(|rival| rival.at_war) {
        out.extend(
            rival
                .units
                .iter()
                .filter(|u| u.combat > 0.0 || u.ranged > 0.0)
                .map(|u| (u.x, u.y)),
        );
    }
    for minor in state.minors.iter().filter(|minor| minor.at_war) {
        out.extend(
            minor
                .units
                .iter()
                .filter(|u| u.combat > 0.0 || u.ranged > 0.0)
                .map(|u| (u.x, u.y)),
        );
    }
    out.extend(
        state
            .hostiles
            .iter()
            .filter(|u| u.combat > 0.0 || u.ranged > 0.0)
            .map(|u| (u.x, u.y)),
    );
    out
}

/// Where each kept General goes this frame: `Some(tile)` to walk there, `None`
/// to hold where it stands. A General absent from the map is not kept and is
/// left to the ordinary activation.
///
/// A Comandante with no post is still kept while fewer than
/// [`COMANDANTES_KEPT`] are: it walks to the soldier that leads the most
/// others (the nearest of equals), away from the posts already taken when it
/// can.
pub(super) fn plan(state: &StateSnapshot) -> BTreeMap<i64, Option<(i32, i32)>> {
    let mut generals: Vec<&StateUnit> = state
        .units
        .iter()
        .filter(|unit| unit.great_person.is_some() && is_general(&unit.kind))
        .collect();
    generals.sort_by_key(|unit| unit.id);
    let mut out = BTreeMap::new();
    if generals.is_empty() {
        return out;
    }
    let soldiers: Vec<&StateUnit> = state.units.iter().filter(|u| is_land_soldier(u)).collect();
    if soldiers.is_empty() {
        return out;
    }
    let enemies = enemy_positions(state);
    // (worth, led, threat, tile) for every soldier's tile, best first; ties
    // break on the tile so the choice is stable frame to frame.
    let mut ranked: Vec<(usize, usize, usize, (i32, i32))> = soldiers
        .iter()
        .map(|anchor| {
            let tile = (anchor.x, anchor.y);
            let led = soldiers
                .iter()
                .filter(|u| offset_distance(tile, (u.x, u.y)) <= AURA_RADIUS)
                .count();
            let threat = enemies
                .iter()
                .filter(|&&e| offset_distance(tile, e) <= THREAT_RADIUS)
                .count()
                .min(4);
            (led + 2 * threat, led, threat, tile)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.3.cmp(&b.3)));
    ranked.dedup_by_key(|post| post.3);
    let posts: Vec<(usize, usize, usize, (i32, i32))> = ranked
        .iter()
        .filter(|&&(_, led, threat, _)| led >= MIN_LED || threat > 0)
        .copied()
        .collect();
    let mut taken: Vec<(i32, i32)> = Vec::new();
    let mut free: Vec<&StateUnit> = generals;
    for &(_, _, _, tile) in &posts {
        if free.is_empty() {
            break;
        }
        if taken
            .iter()
            .any(|&other| offset_distance(tile, other) < FRONT_SEPARATION)
        {
            continue;
        }
        taken.push(tile);
        let (index, general) = free
            .iter()
            .enumerate()
            .min_by_key(|(_, g)| (offset_distance((g.x, g.y), tile), g.id))
            .map(|(i, g)| (i, *g))
            .expect("free is not empty");
        free.remove(index);
        let stand = (general.x, general.y);
        out.insert(
            general.id,
            if offset_distance(stand, tile) <= 1 {
                None
            } else {
                Some(tile)
            },
        );
    }
    // Comandantes past the posts: the first few wait with the army.
    let posted = state
        .units
        .iter()
        .filter(|unit| is_comandante(&unit.kind) && out.contains_key(&unit.id))
        .count();
    let waiting = free
        .into_iter()
        .filter(|g| is_comandante(&g.kind))
        .take(COMANDANTES_KEPT.saturating_sub(posted));
    for general in waiting {
        let stand = (general.x, general.y);
        // The soldier leading the most others, then the nearest one.
        let best = |apart: bool| {
            ranked
                .iter()
                .filter(|post| {
                    !apart
                        || taken
                            .iter()
                            .all(|&other| offset_distance(post.3, other) >= FRONT_SEPARATION)
                })
                .min_by_key(|post| {
                    (
                        std::cmp::Reverse(post.0),
                        offset_distance(stand, post.3),
                        post.3,
                    )
                })
                .map(|post| post.3)
        };
        let Some(tile) = best(true).or_else(|| best(false)) else {
            break;
        };
        taken.push(tile);
        out.insert(
            general.id,
            if offset_distance(stand, tile) <= 1 {
                None
            } else {
                Some(tile)
            },
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use civvis::mirror::{StateGreatPerson, StateRival};

    fn general(id: i64, x: i32, y: i32) -> StateUnit {
        StateUnit {
            id,
            kind: "UNIT_COMANDANTE_GENERAL".into(),
            x,
            y,
            great_person: Some(StateGreatPerson {
                charges: 1,
                can_activate: true,
                ..StateGreatPerson::default()
            }),
            ..StateUnit::default()
        }
    }

    fn soldier(id: i64, x: i32, y: i32) -> StateUnit {
        StateUnit {
            id,
            kind: "UNIT_ARCHER".into(),
            class: Some("PROMOTION_CLASS_RANGED".into()),
            x,
            y,
            combat: 15.0,
            ranged: 25.0,
            ..StateUnit::default()
        }
    }

    fn enemy(x: i32, y: i32) -> StateUnit {
        StateUnit {
            kind: "UNIT_SWORDSMAN".into(),
            class: Some("PROMOTION_CLASS_MELEE".into()),
            x,
            y,
            combat: 35.0,
            ..StateUnit::default()
        }
    }

    fn at_war(units: Vec<StateUnit>) -> StateRival {
        StateRival {
            player: 1,
            at_war: true,
            units,
            ..StateRival::default()
        }
    }

    #[test]
    fn a_general_walks_to_the_largest_cluster() {
        let state = StateSnapshot {
            units: vec![
                general(1, 0, 0),
                soldier(10, 10, 10),
                soldier(11, 11, 10),
                soldier(12, 10, 11),
                soldier(13, 20, 20),
            ],
            ..StateSnapshot::default()
        };
        let plan = plan(&state);
        let target = plan.get(&1).copied().flatten().expect("the general moves");
        assert!(offset_distance(target, (10, 10)) <= 1, "{target:?}");
    }

    #[test]
    fn a_front_outranks_a_larger_quiet_garrison() {
        let state = StateSnapshot {
            units: vec![
                general(1, 0, 0),
                soldier(10, 10, 10),
                soldier(11, 11, 10),
                soldier(12, 10, 11),
                soldier(20, 30, 10),
                soldier(21, 31, 10),
            ],
            rivals: vec![at_war(vec![enemy(33, 10), enemy(33, 11)])],
            ..StateSnapshot::default()
        };
        let target = plan(&state).get(&1).copied().flatten().expect("moves");
        assert!(offset_distance(target, (30, 10)) <= 1, "{target:?}");
    }

    #[test]
    fn a_general_beside_its_post_holds() {
        let state = StateSnapshot {
            units: vec![general(1, 10, 10), soldier(10, 10, 10), soldier(11, 11, 10)],
            ..StateSnapshot::default()
        };
        assert_eq!(plan(&state).get(&1), Some(&None));
        let (orders, _) = super::super::great_person_orders_led(&state, true);
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].verb.as_deref(), Some("KEEP_GREAT_PERSON"));
    }

    #[test]
    fn a_second_general_takes_a_second_front_and_a_third_is_left_free() {
        let state = StateSnapshot {
            units: vec![
                general(1, 0, 0),
                general(2, 40, 0),
                general(3, 20, 0),
                soldier(10, 2, 2),
                soldier(11, 3, 2),
                soldier(20, 40, 2),
                soldier(21, 41, 2),
            ],
            ..StateSnapshot::default()
        };
        let plan = plan(&state);
        assert_eq!(plan.len(), 2, "{plan:?}");
        assert!(plan.contains_key(&1) && plan.contains_key(&2), "{plan:?}");
        assert!(
            !plan.contains_key(&3),
            "the surplus General is left to retire"
        );
    }

    fn great_general(id: i64, x: i32, y: i32) -> StateUnit {
        StateUnit {
            kind: "UNIT_GREAT_GENERAL".into(),
            ..general(id, x, y)
        }
    }

    #[test]
    fn a_lone_soldier_with_no_enemy_is_no_post() {
        let state = StateSnapshot {
            units: vec![great_general(1, 0, 0), soldier(10, 10, 10)],
            ..StateSnapshot::default()
        };
        assert!(plan(&state).is_empty(), "a Great General is left to retire");
    }

    #[test]
    fn a_comandante_waits_with_a_scattered_army() {
        // No two soldiers within two tiles and no enemy: no post, as on live
        // turn 44. The Comandante walks to a soldier instead of retiring.
        let state = StateSnapshot {
            units: vec![
                general(1, 0, 0),
                soldier(10, 10, 10),
                soldier(11, 20, 10),
                soldier(12, 30, 10),
            ],
            ..StateSnapshot::default()
        };
        let plan = plan(&state);
        let target = plan.get(&1).copied().flatten().expect("kept and walking");
        assert_eq!(target, (10, 10), "the nearest of the equal tiles");

        // Standing with a lone soldier, it holds.
        let state = StateSnapshot {
            units: vec![general(1, 10, 10), soldier(10, 10, 10)],
            ..StateSnapshot::default()
        };
        assert_eq!(plan_of(&state).get(&1), Some(&None));
    }

    #[test]
    fn only_two_comandantes_wait_and_they_stand_apart() {
        let state = StateSnapshot {
            units: vec![
                general(1, 0, 0),
                general(2, 0, 1),
                general(3, 0, 2),
                soldier(10, 10, 10),
                soldier(11, 30, 10),
            ],
            ..StateSnapshot::default()
        };
        let plan = plan(&state);
        assert_eq!(plan.len(), 2, "{plan:?}");
        let mut tiles: Vec<(i32, i32)> = plan.values().filter_map(|tile| *tile).collect();
        tiles.sort_unstable();
        assert_eq!(tiles, vec![(10, 10), (30, 10)]);
        assert!(!plan.contains_key(&3), "the third is left to retire");
    }

    fn plan_of(state: &StateSnapshot) -> BTreeMap<i64, Option<(i32, i32)>> {
        plan(state)
    }

    #[test]
    fn naval_units_and_civilians_are_not_led() {
        let mut galley = soldier(10, 10, 10);
        galley.class = Some("PROMOTION_CLASS_NAVAL_MELEE".into());
        let mut builder = soldier(11, 11, 10);
        builder.class = None;
        builder.combat = 0.0;
        builder.ranged = 0.0;
        let state = StateSnapshot {
            units: vec![general(1, 0, 0), galley, builder],
            ..StateSnapshot::default()
        };
        assert!(plan(&state).is_empty());
    }

    #[test]
    fn a_great_prophet_is_not_a_general() {
        assert!(is_general("UNIT_GREAT_GENERAL"));
        assert!(is_general("UNIT_COMANDANTE_GENERAL"));
        assert!(!is_general("UNIT_GREAT_PROPHET"));
        assert!(!is_general("UNIT_GREAT_ADMIRAL"));
    }
}
