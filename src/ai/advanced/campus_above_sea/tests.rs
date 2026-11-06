use super::*;

fn board() -> (Game, AdvancedAi, u32, Pos, Pos) {
    let mut g = Game::new_full(1, 24, 16, 91_989, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let cid = g.found_city_for(0, g.units[&settler].pos, None);
    g.remove_unit(settler);
    g.cities.get_mut(&cid).unwrap().pop = 9;
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    let sites = g.district_sites(cid, crate::name!("campus"));
    assert!(sites.len() >= 2);
    for pos in &sites {
        g.map.tiles.get_mut(pos).unwrap().coastal_lowland = 1;
    }
    let (low, safe) = (sites[0], sites[1]);
    g.map.tiles.get_mut(&safe).unwrap().coastal_lowland = 0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_campus_above_the_sea();
    (g, ai, cid, low, safe)
}

#[test]
fn a_fresh_science_campus_uses_a_safe_legal_site_in_the_same_city() {
    let (g, ai, cid, low, safe) = board();
    assert_eq!(
        ai.native_campus_above_sea_position(&g, 0, cid, low),
        Some(safe)
    );
    assert!(g.can_produce(
        0,
        cid,
        &Item::District {
            district: crate::name!("campus"),
            pos: safe
        }
    ));
    assert_eq!(ai.native_campus_above_sea_position(&g, 1, cid, low), None);
}

#[test]
fn no_safe_site_or_an_already_safe_proposal_keeps_the_existing_choice() {
    let (mut g, ai, cid, low, safe) = board();
    assert_eq!(ai.native_campus_above_sea_position(&g, 0, cid, safe), None);
    g.map.tiles.get_mut(&safe).unwrap().coastal_lowland = 3;
    assert_eq!(ai.native_campus_above_sea_position(&g, 0, cid, low), None);
}

#[test]
fn an_active_barrier_keeps_the_original_site_but_a_pillaged_one_does_not() {
    let (mut g, ai, cid, low, safe) = board();
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("flood_barrier"));
    assert_eq!(ai.native_campus_above_sea_position(&g, 0, cid, low), None);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("flood_barrier"));
    assert_eq!(
        ai.native_campus_above_sea_position(&g, 0, cid, low),
        Some(safe)
    );
}

#[test]
fn an_existing_queue_or_paused_foundation_is_never_relocated() {
    let (mut g, ai, cid, low, _) = board();
    g.cities.get_mut(&cid).unwrap().queue.push(Item::District {
        district: crate::name!("campus"),
        pos: low,
    });
    assert_eq!(ai.native_campus_above_sea_position(&g, 0, cid, low), None);
    g.cities.get_mut(&cid).unwrap().queue.clear();
    g.map.tiles.get_mut(&low).unwrap().district_foundation =
        Some(crate::world::DistrictFoundation {
            district: crate::name!("campus"),
            cost: 56.0,
        });
    assert_eq!(ai.native_campus_above_sea_position(&g, 0, cid, low), None);
}

#[test]
fn the_gene_is_off_in_both_controllers_and_other_lanes_keep_their_sites() {
    super::super::test_support::opt_in_off_in_both_controllers("campus-above-the-sea", |ai| {
        ai.campus_above_the_sea
    });
    let (g, mut ai, cid, low, _) = board();
    ai.disable_campus_above_the_sea();
    assert_eq!(ai.native_campus_above_sea_position(&g, 0, cid, low), None);
    let mut domination = AdvancedAi::targeting(VictoryTarget::Domination);
    domination.enable_campus_above_the_sea();
    assert_eq!(
        domination.native_campus_above_sea_position(&g, 0, cid, low),
        None
    );
}

/// Like the native callback, this calls the public retargeter on a fresh
/// mirror world: no AI turn has primed its district-ranking context.
fn fresh_indonesia() -> (Game, AdvancedAi, u32, Pos, Pos, Pos) {
    let mut g = Game::new_full(1, 24, 16, 91_989, 250, 0, false);
    g.players[0].civ = "Indonesia".to_string();
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let cid = g.found_city_for(0, g.units[&settler].pos, None);
    g.remove_unit(settler);
    g.cities.get_mut(&cid).unwrap().pop = 9;
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.coastal_lowland = 1;
    }
    let mut sites = g.district_sites(cid, crate::name!("campus"));
    sites.sort();
    let center = g.cities[&cid].pos;
    for &coastal in &sites {
        for &plain in &sites {
            if coastal >= plain || g.nbrs(coastal).contains(&plain) {
                continue;
            }
            for &low in &sites {
                if low == coastal || low == plain {
                    continue;
                }
                let plain_neighbors = g.nbrs(plain);
                let coast: Vec<_> = g
                    .nbrs(coastal)
                    .into_iter()
                    .filter(|p| *p != center && *p != low && !plain_neighbors.contains(p))
                    .take(2)
                    .collect();
                if coast.len() != 2 {
                    continue;
                }
                let mut candidate = g.clone();
                for pos in coast {
                    candidate.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
                }
                for pos in [coastal, plain] {
                    candidate.map.tiles.get_mut(&pos).unwrap().coastal_lowland = 0;
                    assert!(candidate.can_produce(
                        0,
                        cid,
                        &Item::District {
                            district: crate::name!("campus"),
                            pos,
                        }
                    ));
                }
                if candidate
                    .district_yields(crate::name!("campus"), coastal)
                    .total()
                    != candidate
                        .district_yields(crate::name!("campus"), plain)
                        .total()
                {
                    continue;
                }
                let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
                ai.enable_campus_above_the_sea();
                return (candidate, ai, cid, low, coastal, plain);
            }
        }
    }
    panic!("fixture needs two equally ranked legal safe sites with distinct Coast adjacency");
}

#[test]
fn fresh_mirror_safe_campus_honors_nusantara_when_both_genes_are_on() {
    let (mut g, mut ai, cid, low, coastal, plain) = fresh_indonesia();
    assert_eq!(g.nusantara_site_planner, None);
    assert_eq!(
        ai.native_campus_above_sea_position(&g, 0, cid, low),
        Some(plain),
        "with Nusantara off, retain the ordinary positional tie-break"
    );
    ai.enable_nusantara_reads_the_coast();
    assert_eq!(
        ai.native_campus_above_sea_position(&g, 0, cid, low),
        Some(coastal),
        "the fresh native mirror must price two adjacent Coast tiles among safe offers"
    );
    assert_eq!(
        g.nusantara_site_planner, None,
        "the caller's board stays unchanged"
    );
    ai.disable_nusantara_reads_the_coast();
    g.nusantara_site_planner = Some(0);
    assert_eq!(
        ai.native_campus_above_sea_position(&g, 0, cid, low),
        Some(plain),
        "with the gene off, ignore stale planner context on the caller's board"
    );
}

#[test]
fn a_safe_legal_site_outside_the_two_site_production_shortlist_is_considered() {
    let (mut g, ai, cid, _, _, _) = fresh_indonesia();
    let district = crate::name!("campus");
    let sites = g.district_sites(cid, district);
    for pos in &sites {
        g.map.tiles.get_mut(pos).unwrap().coastal_lowland = 1;
    }
    let shortlisted: Vec<_> = g
        .producible_items(0, cid)
        .into_iter()
        .filter_map(|item| match item {
            Item::District {
                district: offered,
                pos,
            } if offered == district => Some(pos),
            _ => None,
        })
        .collect();
    assert_eq!(shortlisted.len(), 2);
    let safe = *sites.iter().find(|pos| !shortlisted.contains(pos)).unwrap();
    g.map.tiles.get_mut(&safe).unwrap().coastal_lowland = 0;
    assert!(g.can_produce(
        0,
        cid,
        &Item::District {
            district,
            pos: safe
        }
    ));
    assert_eq!(
        ai.native_campus_above_sea_position(&g, 0, cid, shortlisted[0]),
        Some(safe)
    );
}
