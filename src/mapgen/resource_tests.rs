use super::*;

const SCRIPTS: [MapScript; 4] = [
    MapScript::Continents,
    MapScript::Pangaea,
    MapScript::Islands,
    MapScript::Lakes,
];

fn standard_world(rules: &Rules, script: MapScript, seed: u64) -> WorldMap {
    let mut rng = Rng::new(seed);
    generate_with_script(
        rules,
        84,
        54,
        8,
        12,
        5,
        4,
        script,
        MapTopology::Flat,
        MapPoles::Poles,
        &mut rng,
    )
    .0
}

fn is_class(rules: &Rules, resource: Option<Name>, class: &str) -> bool {
    resource.is_some_and(|resource| rules.resources[&resource].class == class)
}

/// One luxury deck is dealt four kinds at a time, so a land luxury kind
/// belongs to one continent. Revealed plots from 101 live games agree: 90 of
/// 106 well-explored continents showed exactly 4 kinds and none showed more.
#[test]
fn each_continent_is_dealt_its_own_four_luxury_kinds() {
    let rules = Rules::embedded();
    for (index, script) in SCRIPTS.into_iter().enumerate() {
        let world = standard_world(&rules, script, 94_000 + index as u64);
        let mut kinds: BTreeMap<usize, BTreeSet<Name>> = BTreeMap::new();
        for tile in world.tiles.values() {
            if !rules.is_water(tile) && is_class(&rules, tile.resource, "luxury") {
                kinds
                    .entry(tile.continent.expect("land has a continent"))
                    .or_default()
                    .insert(tile.resource.unwrap());
            }
        }
        assert!(!kinds.is_empty(), "{script:?} placed no land luxuries");
        let mut seen = BTreeSet::new();
        for (continent, held) in &kinds {
            assert!(
                held.len() <= 4,
                "{script:?} continent {continent} holds {} kinds",
                held.len()
            );
            for kind in held {
                assert!(
                    seen.insert(*kind),
                    "{script:?} put {kind} on two continents"
                );
            }
        }
    }
}

/// A continent's strategics follow its size and each kind's `Frequency`, with
/// one deposit of a kind guaranteed where a plot can take it.
#[test]
fn strategic_deposits_follow_each_continents_size() {
    let rules = Rules::embedded();
    let total: f64 = rules
        .resources
        .values()
        .filter(|spec| spec.class == "strategic")
        .map(|spec| spec.frequency as f64)
        .sum();
    assert_eq!(total, 63.0, "the Gathering Storm Frequency column");
    let world = standard_world(&rules, MapScript::Continents, 94_100);
    let mut land: BTreeMap<usize, usize> = BTreeMap::new();
    let mut held: BTreeMap<(usize, Name), usize> = BTreeMap::new();
    for tile in world.tiles.values().filter(|tile| !rules.is_water(tile)) {
        let continent = tile.continent.expect("land has a continent");
        *land.entry(continent).or_default() += 1;
        if is_class(&rules, tile.resource, "strategic") {
            *held.entry((continent, tile.resource.unwrap())).or_default() += 1;
        }
    }
    assert!(land.len() <= 7, "the distribution weights cover seven");
    for (continent, plots) in &land {
        for (name, spec) in rules
            .resources
            .iter()
            .filter(|(_, spec)| spec.class == "strategic")
        {
            let share = *plots as f64 * 0.30 * 0.217 * spec.frequency as f64 / total;
            let limit = (share as usize).max(1);
            let found = held.get(&(*continent, *name)).copied().unwrap_or(0);
            assert!(
                found <= limit,
                "continent {continent} of {plots} plots holds {found} {name}, limit {limit}"
            );
        }
    }
    let placed: usize = held.values().sum();
    let wanted = land.values().sum::<usize>() as f64 * 0.30 * 0.217;
    assert!(
        placed as f64 > wanted * 0.6,
        "{placed} deposits against a target of {wanted}"
    );
}

/// The shares Standard Resources aims for, against what 101 live Continents
/// games show: 5.4 percent of land holds a luxury, 9.1 percent a bonus.
#[test]
fn land_and_sea_fill_the_shipped_shares() {
    let rules = Rules::embedded();
    let mut land = 0usize;
    let mut water = 0usize;
    let mut luxury = 0usize;
    let mut bonus = 0usize;
    let mut sea_bonus = 0usize;
    for seed in 0..4 {
        let world = standard_world(&rules, MapScript::Continents, 94_200 + seed);
        for tile in world.tiles.values() {
            if rules.is_water(tile) {
                water += 1;
                sea_bonus += usize::from(is_class(&rules, tile.resource, "bonus"));
            } else {
                land += 1;
                luxury += usize::from(is_class(&rules, tile.resource, "luxury"));
                bonus += usize::from(is_class(&rules, tile.resource, "bonus"));
            }
        }
    }
    let share = |count: usize, of: usize| 100.0 * count as f64 / of as f64;
    let luxury = share(luxury, land);
    let bonus = share(bonus, land);
    let sea_bonus = share(sea_bonus, water);
    assert!((4.0..7.5).contains(&luxury), "luxury {luxury:.2}% of land");
    assert!((7.0..14.0).contains(&bonus), "bonus {bonus:.2}% of land");
    assert!(
        (3.5..10.0).contains(&sea_bonus),
        "bonus {sea_bonus:.2}% of water"
    );
}

/// The shipped `Resource_ValidTerrains` and `Resource_ValidFeatures` rows, and
/// the `LakeEligible` and `AdjacentToLand` flags.
#[test]
fn the_shipped_rows_decide_where_a_resource_can_lie() {
    let rules = Rules::embedded();
    let mut wm = WorldMap::new(12, 8);
    let center = hex::offset_to_axial(5, 4);
    let neighbors: Vec<Pos> = wm.neighbors(center).into_iter().collect();
    for pos in wm.tiles.keys().copied().collect::<Vec<_>>() {
        wm.tiles.get_mut(&pos).unwrap().terrain = crate::name!("ocean");
    }
    let mut check =
        |terrain: &str, hills: bool, feature: Option<&str>, resource: &str, can: bool| {
            let tile = wm.tiles.get_mut(&center).unwrap();
            tile.terrain = Name::new(terrain);
            tile.hills = hills;
            tile.feature = feature.map(Name::new);
            tile.resource = None;
            assert_eq!(
                can_have_resource(&rules, &wm, center, Name::new(resource)),
                can,
                "{resource} on {terrain} hills={hills} feature={feature:?}"
            );
        };
    // A listed feature settles it, whatever lies beneath.
    check("grassland", false, Some("forest"), "coal", true);
    check("tundra", false, Some("forest"), "coal", true);
    check("grassland", false, None, "coal", false);
    check("grassland", true, None, "coal", true);
    check("grassland", false, Some("marsh"), "rice", true);
    check("grassland", false, Some("marsh"), "sugar", true);
    check("grassland", false, Some("oasis"), "rice", false);
    // A feature that is not listed bars the plot.
    check("grassland", true, Some("forest"), "stone", false);
    // Forms of one terrain that the rows split.
    check("plains", false, None, "aluminum", true);
    check("plains", true, None, "aluminum", false);
    check("desert", true, None, "aluminum", true);
    check("plains", false, None, "gypsum", true);
    check("desert", false, None, "gypsum", false);
    check("tundra", false, None, "gypsum", false);
    check("desert", true, None, "gypsum", true);
    check("desert", false, None, "ivory", true);
    check("desert", true, None, "ivory", false);
    check("plains", false, None, "marble", false);
    check("plains", true, None, "marble", true);
    check("grassland", false, None, "marble", true);
    // Lakes are shallow water to every resource but Whales.
    check("lake", false, None, "fish", true);
    check("lake", false, None, "crabs", true);
    check("lake", false, None, "pearls", true);
    check("lake", false, None, "oil", true);
    check("lake", false, None, "whales", false);
    check("coast", false, None, "whales", true);
    // Amber has to touch land, which no neighbour here is yet.
    check("coast", false, None, "amber", false);
    wm.tiles.get_mut(&neighbors[0]).unwrap().terrain = crate::name!("plains");
    let tile = wm.tiles.get_mut(&center).unwrap();
    tile.terrain = crate::name!("coast");
    tile.hills = false;
    tile.feature = None;
    assert!(can_have_resource(&rules, &wm, center, Name::new("amber")));
}

/// `__RemoveDuplicateResources`: two neighbours of the same bonus kind remove
/// a plot, and the scan sees its own earlier removals.
#[test]
fn a_bonus_resource_crowded_by_its_own_kind_is_removed() {
    let rules = Rules::embedded();
    let mut wm = WorldMap::new(12, 8);
    let center = hex::offset_to_axial(5, 4);
    let neighbors: Vec<Pos> = wm.neighbors(center).into_iter().collect();
    for pos in wm.tiles.keys().copied().collect::<Vec<_>>() {
        wm.tiles.get_mut(&pos).unwrap().terrain = crate::name!("plains");
    }
    let wheat = Name::new("wheat");
    for pos in [center, neighbors[0], neighbors[3]] {
        wm.tiles.get_mut(&pos).unwrap().resource = Some(wheat);
    }
    let count = |wm: &WorldMap| {
        wm.tiles
            .values()
            .filter(|tile| tile.resource == Some(wheat))
            .count()
    };
    let mut plots: Vec<Pos> = wm.tiles.keys().copied().collect();
    plots.sort_by_key(|&(q, r)| hex::axial_to_offset(q, r));
    thin_crowded_bonus(&rules, &mut wm, &plots);
    assert_eq!(count(&wm), 2, "the center is the only crowded plot");
    assert_eq!(wm.tiles[&center].resource, None);

    // Strategics and luxuries may crowd together.
    let iron = Name::new("iron");
    for pos in [center, neighbors[0], neighbors[3]] {
        wm.tiles.get_mut(&pos).unwrap().resource = Some(iron);
    }
    thin_crowded_bonus(&rules, &mut wm, &plots);
    assert_eq!(wm.tiles[&center].resource, Some(iron));
}
