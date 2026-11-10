use super::*;

/// Two cities of ours with a Theater and an Amphitheater each, every civic
/// and tech known, an Archaeological Museum in the first, an empty Artifact
/// slot, and no Antiquity Site on the map yet.
fn fixture(gene: bool) -> (AdvancedAi, Game, Vec<u32>) {
    let (mut g, cities) =
        crate::ai::industrial_zones::tests::producer_empire("DIGSEVERYSITE", 436_000, 2);
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    g.players[0].civics = g.rules.civics.keys().copied().collect();
    for cid in &cities {
        let position = g.cities[cid]
            .owned_tiles
            .iter()
            .copied()
            .find(|pos| *pos != g.cities[cid].pos && g.map.tiles[pos].district.is_none())
            .unwrap();
        g.map.tiles.get_mut(&position).unwrap().district = Some(crate::name!("theater_square"));
        let city = g.cities.get_mut(cid).unwrap();
        city.districts
            .insert(crate::name!("theater_square"), position);
        city.buildings.push(crate::name!("amphitheater"));
        city.queue.clear();
    }
    g.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(crate::name!("archaeological_museum"));
    for tile in g.map.tiles.values_mut() {
        if matches!(
            tile.resource.as_deref(),
            Some("antiquity_site" | "shipwreck")
        ) {
            tile.resource = None;
        }
    }
    g.players[0]
        .counters
        .insert("great_work:artifact".into(), 0);
    g.players[0].live_open_great_work_slots = Some(["artifact".into()].into_iter().collect());
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    if gene {
        ai.enable_culture_digs_every_site();
    }
    (ai, g, cities)
}

/// Bury `count` Antiquity Sites on plain land our cities own, the first
/// city's tiles first and each city's nearest its centre first, and return
/// them.
fn bury(g: &mut Game, cities: &[u32], count: usize) -> Vec<Pos> {
    let mut land: Vec<Pos> = Vec::new();
    for city in cities {
        let centre = g.cities[city].pos;
        let mut own: Vec<Pos> = g.cities[city]
            .owned_tiles
            .iter()
            .copied()
            .filter(|pos| {
                *pos != centre
                    && g.map.tiles[pos].district.is_none()
                    && g.map.tiles[pos].resource.is_none()
                    && g.city_at(*pos).is_none()
                    && !g.rules.is_water(&g.map.tiles[pos])
            })
            .collect();
        own.sort_by_key(|pos| (g.wdist(centre, *pos), *pos));
        land.extend(own);
    }
    land.truncate(count);
    assert_eq!(land.len(), count, "fixture: room for the sites");
    for site in &land {
        let tile = g.map.tiles.get_mut(site).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = Some(crate::name!("antiquity_site"));
        tile.improvement = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.flooded = false;
        tile.submerged = false;
    }
    land
}

fn digger() -> Item {
    Item::Unit {
        unit: crate::name!("archaeologist"),
    }
}

#[test]
fn culture_digs_every_site_is_reversible_and_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "culture-digs-every-site",
        |ai| ai.culture_digs_every_site,
    );
}

#[test]
fn the_museums_follow_the_reachable_sites() {
    let (ai, mut g, cities) = fixture(true);
    assert_eq!(
        ai.dig_museums_wanted(&g, 0),
        1,
        "no site visible: one Museum"
    );
    bury(&mut g, &cities, 3);
    assert_eq!(AdvancedAi::reachable_dig_sites(&g, 0), 3);
    assert_eq!(
        ai.dig_museums_wanted(&g, 0),
        1,
        "three sites fill one Museum"
    );
    let (ai, mut g, cities) = fixture(true);
    bury(&mut g, &cities, 4);
    assert_eq!(
        ai.dig_museums_wanted(&g, 0),
        2,
        "a fourth site wants a second"
    );
    let (ai, mut g, cities) = fixture(true);
    bury(&mut g, &cities, 8);
    assert_eq!(ai.dig_museums_wanted(&g, 0), DIG_MUSEUM_CAP, "capped");

    // Before Natural History reveals the sites the gene keeps one Museum.
    g.players[0].civics.remove(&crate::name!("natural_history"));
    assert_eq!(ai.dig_museums_wanted(&g, 0), 1);
}

#[test]
fn off_or_another_target_wants_no_museum() {
    let (ai, mut g, cities) = fixture(false);
    bury(&mut g, &cities, 6);
    assert_eq!(ai.dig_museums_wanted(&g, 0), 0, "gene off");
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_culture_digs_every_site();
    assert_eq!(science.dig_museums_wanted(&g, 0), 0, "science seat");
}

#[test]
fn the_museum_bonus_lasts_until_the_wanted_count_stands() {
    let dig = crate::name!("archaeological_museum");
    let (ai, mut g, cities) = fixture(true);
    bury(&mut g, &cities, 5);
    assert!(!ai.culture_digs_one_museum, "carried without the old gene");
    assert_eq!(
        ai.dig_museum_bonus(&g, 0, dig),
        crate::ai::advanced::culture_dig_museum::DIG_MUSEUM_BONUS,
        "one Museum of two"
    );
    assert_eq!(ai.dig_museum_bonus(&g, 0, crate::name!("art_museum")), 0.0);
    g.cities
        .get_mut(&cities[1])
        .unwrap()
        .queue
        .push(Item::Building { building: dig });
    assert_eq!(ai.dig_museum_bonus(&g, 0, dig), 0.0, "the second is queued");

    let (ai, mut g, cities) = fixture(true);
    bury(&mut g, &cities, 2);
    assert_eq!(
        ai.dig_museum_bonus(&g, 0, dig),
        0.0,
        "two sites: one Museum"
    );
}

#[test]
fn the_museum_city_claims_the_archaeologist() {
    let (ai, mut g, cities) = fixture(true);
    bury(&mut g, &cities, 2);
    assert!(g.can_produce(0, cities[0], &digger()));
    let plan = ai.assess(&g, 0);
    assert!(
        ai.dig_archaeologist_item(&g, 0, cities[1], &plan).is_none(),
        "no Museum"
    );
    ai.claim_dig_archaeologist(&mut g, 0, &plan);
    assert_eq!(g.cities[&cities[0]].queue.first(), Some(&digger()));
    assert!(ai.dig_archaeologist_holds(&g, 0, cities[0], &plan, &digger()));

    // One at a time: an Archaeologist afield ends the claim and the hold.
    let at = g.cities[&cities[0]].pos;
    g.spawn_test_unit("archaeologist", 0, at);
    assert!(ai.dig_archaeologist_item(&g, 0, cities[0], &plan).is_none());
    assert!(!ai.dig_archaeologist_holds(&g, 0, cities[0], &plan, &digger()));
}

#[test]
fn no_slot_no_site_or_gene_off_claims_nothing() {
    let (ai, mut g, cities) = fixture(true);
    let plan = ai.assess(&g, 0);
    ai.claim_dig_archaeologist(&mut g, 0, &plan);
    assert!(g.cities[&cities[0]].queue.is_empty(), "no site");

    let (ai, mut g, cities) = fixture(true);
    bury(&mut g, &cities, 2);
    g.players[0].live_open_great_work_slots = Some(Default::default());
    let plan = ai.assess(&g, 0);
    ai.claim_dig_archaeologist(&mut g, 0, &plan);
    assert!(
        g.cities[&cities[0]].queue.is_empty(),
        "the host lists no slot"
    );

    let (ai, mut g, cities) = fixture(false);
    bury(&mut g, &cities, 2);
    let plan = ai.assess(&g, 0);
    ai.claim_dig_archaeologist(&mut g, 0, &plan);
    assert!(g.cities[&cities[0]].queue.is_empty(), "gene off");
    assert!(!ai.dig_archaeologist_holds(&g, 0, cities[0], &plan, &digger()));
}

#[test]
fn own_sites_come_before_unowned_and_foreign_ones() {
    let (_, g, cities) = fixture(true);
    let centre = g.cities[&cities[0]].pos;
    let own = g.cities[&cities[0]]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != centre)
        .unwrap();
    let unowned = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.map.tiles[pos].owner_city.is_none())
        .unwrap();
    let own_rank = AdvancedAi::dig_site_rank(&g, 0, centre, own);
    let unowned_rank = AdvancedAi::dig_site_rank(&g, 0, centre, unowned);
    assert_eq!(own_rank.0, 0);
    assert_eq!(unowned_rank.0, 1);
    assert!(own_rank < unowned_rank);
}

#[test]
fn a_barred_site_is_passed_over() {
    let (mut ai, mut g, cities) = fixture(true);
    let sites = bury(&mut g, &cities, 1);
    let at = g.cities[&cities[0]].pos;
    let uid = g.spawn_test_unit("archaeologist", 0, at);
    g.current = 0;
    let mut barred = g.clone();
    let mut barred_ai = ai.clone();
    barred_ai.dig_dead_sites.insert(sites[0], g.turn + 10);
    assert!(
        !barred_ai.dig_every_site_step(&mut barred, 0, uid),
        "the only site is barred"
    );
    assert!(
        ai.dig_every_site_step(&mut g, 0, uid),
        "the open site is walked to"
    );
}
